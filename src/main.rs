#![feature(async_await, await_macro, futures_api)]

use std::cmp;
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use futures01::future::Future as Future01;
use futures01::sync::mpsc::channel;
use serde_derive::Deserialize;

use components_database::rocks::RocksDB;
use components_executor::evm::{EVMBlockDataProvider, EVMExecutor};
use components_executor::TrieDB;
use components_jsonrpc;
use components_transaction_pool::HashTransactionPool;
use core_consensus::{Bft, ConsensusStatus, Engine, SynchronizerManager};
use core_context::Context;
use core_crypto::{
    secp256k1::{PrivateKey, Secp256k1},
    Crypto, CryptoTransform,
};
use core_network::reactor::{outbound, CallbackMap, InboundReactor, JoinReactor, OutboundReactor};
use core_network::{Config as NetworkConfig, Network};
use core_pubsub::{PubSub, PUBSUB_BROADCAST_BLOCK};
use core_storage::{BlockStorage, Storage};
use core_types::{Address, Block, BlockHeader, Genesis, Hash, Proof};
use logger;

#[derive(Debug, Deserialize)]
struct Config {
    // crypto
    privkey: String,

    // rpc_address
    rpc_address: String,
    rpc_workers: u64,

    // db config
    data_path: PathBuf,

    // network
    private_key:         Option<String>,
    bootstrap_addresses: Vec<String>,
    listening_address:   String,

    // transaction pool
    pool_size:         u64,
    until_block_limit: u64,
    quota_limit:       u64,

    // consensus
    consensus_tx_limit:      u64,
    consensus_interval:      u64,
    consensus_verifier_list: Vec<String>,
    consensus_wal_path:      String,

    // synchronizer
    synchronzer_broadcast_status_interval: u64,
}

impl Config {
    pub fn data_path_for_state(&self) -> PathBuf {
        let mut path_state = self.data_path.clone();
        path_state.push("state_data");
        path_state
    }

    pub fn data_path_for_block(&self) -> PathBuf {
        let mut path_state = self.data_path.clone();
        path_state.push("block_data");
        path_state
    }
}

fn main() {
    logger::init(logger::Flag::Main);
    let matches = clap::App::new("Muta")
        .version("0.1")
        .author("Cryptape Technologies <contact@cryptape.com>")
        .arg(
            clap::Arg::from_usage("-c --config=[FILE] 'a required file for the configuration'")
                .default_value("./devtools/chain/config.toml"),
        )
        .subcommand(
            clap::SubCommand::with_name("init")
                .about("Initializes a new genesis block and definition for the network")
                .arg(
                    clap::Arg::from_usage("<genesis.json> 'expects a genesis file'")
                        .default_value("./devtools/chain/genesis.json"),
                ),
        )
        .get_matches();

    let args_config = matches.value_of("config").unwrap();
    let cfg: Config = config_parser::parse(args_config).unwrap();
    log::info!("Go with config: {:?}", cfg);

    // init genesis
    if let Some(matches) = matches.subcommand_matches("init") {
        let genesis_path = matches.value_of("genesis.json").unwrap();
        log::info!("Genesis path: {}", genesis_path);
        handle_init(&cfg, genesis_path).unwrap();
    }

    start(&cfg);
}

fn start(cfg: &Config) {
    // new context
    let ctx = Context::new();

    // new crypto
    let secp = Arc::new(Secp256k1::new());

    // new db
    let block_db = Arc::new(RocksDB::new(cfg.data_path_for_block().to_str().unwrap()).unwrap());
    let state_db = Arc::new(RocksDB::new(cfg.data_path_for_state().to_str().unwrap()).unwrap());

    // new storage and trie db
    let storage = Arc::new(BlockStorage::new(Arc::clone(&block_db)));
    let trie_db = Arc::new(TrieDB::new(Arc::clone(&state_db)));

    // new executor
    let block = storage.get_latest_block(ctx.clone()).wait().unwrap();
    let executor = Arc::new(
        EVMExecutor::from_existing(
            trie_db,
            Arc::new(EVMBlockDataProvider::new(Arc::clone(&storage))),
            &block.header.state_root,
        )
        .unwrap(),
    );

    let (outbound_tx, outbound_rx) = channel(255);
    let outbound_tx = outbound::Sender::new(outbound_tx);

    // new tx pool
    let tx_pool = Arc::new(HashTransactionPool::new(
        Arc::clone(&storage),
        Arc::clone(&secp),
        outbound_tx.clone(),
        cfg.pool_size as usize,
        cfg.until_block_limit,
        cfg.quota_limit,
    ));

    // run json rpc
    let mut jrpc_config = components_jsonrpc::Config::default();
    jrpc_config.listen = cfg.rpc_address.clone();
    jrpc_config.workers = if cfg.rpc_workers != 0 {
        cfg.rpc_workers as usize
    } else {
        cmp::min(2, num_cpus::get())
    };
    let jrpc_state = components_jsonrpc::AppState::new(
        Arc::clone(&executor),
        Arc::clone(&tx_pool),
        Arc::clone(&storage),
    );

    // new consensus
    let privkey = PrivateKey::from_bytes(&hex::decode(cfg.privkey.clone()).unwrap()).unwrap();

    let pubkey = secp.get_public_key(&privkey).unwrap();
    let node_address = secp.pubkey_to_address(&pubkey);

    let mut verifier_list = Vec::with_capacity(cfg.consensus_verifier_list.len());
    for address in cfg.consensus_verifier_list.iter() {
        verifier_list.push(Address::from_hex(address).unwrap());
    }

    let proof = storage.get_latest_proof(ctx.clone()).wait().unwrap();
    let status = ConsensusStatus {
        height: block.header.height,
        timestamp: block.header.timestamp,
        block_hash: block.hash.clone(),
        state_root: block.header.state_root.clone(),
        tx_limit: cfg.consensus_tx_limit,
        quota_limit: cfg.quota_limit,
        interval: cfg.consensus_interval,
        proof,
        node_address,
        verifier_list,
    };

    let mut pubsub = PubSub::builder().build().start();

    let engine = Arc::new(
        Engine::new(
            Arc::clone(&executor),
            Arc::clone(&tx_pool),
            Arc::clone(&storage),
            Arc::clone(&secp),
            privkey.clone(),
            status,
            pubsub.register(),
        )
        .unwrap(),
    );

    // start consensus.
    let consensus = Bft::new(
        Arc::clone(&engine),
        outbound_tx.clone(),
        &cfg.consensus_wal_path,
    )
    .unwrap();
    let consensus = Arc::new(consensus);

    // net network
    let callback_map = CallbackMap::default();
    let inbound_reactor = InboundReactor::new(
        Arc::clone(&tx_pool),
        Arc::clone(&storage),
        Arc::clone(&engine),
        Arc::new(outbound_tx.clone()),
        Arc::clone(&consensus),
        Arc::clone(&callback_map),
    );
    let outbound_reactor = OutboundReactor::new(callback_map);
    let network_reactor = inbound_reactor.join(outbound_reactor);
    // or
    // let network_reactor = outbound_reactor.join(inbound_reactor);
    // or peer that only handle inbound message
    // let network_reactor = inbound_reactor;
    // or peer that only handle outbound message
    // let network_reactor = outbound_reactor;

    let mut net_config = NetworkConfig::default();
    net_config.p2p.private_key = cfg.private_key.clone();
    net_config.p2p.listening_address = Some(cfg.listening_address.to_owned());
    net_config.p2p.bootstrap_addresses = cfg.bootstrap_addresses.to_owned();
    let _network = Network::new(net_config, outbound_rx, network_reactor).unwrap();

    // start synchronizer
    let sub_block2 = pubsub
        .subscribe::<Block>(PUBSUB_BROADCAST_BLOCK.to_owned())
        .unwrap();
    let synchronizer_manager = SynchronizerManager::new(
        Arc::new(outbound_tx.clone()),
        Arc::clone(&storage),
        cfg.synchronzer_broadcast_status_interval,
    );
    synchronizer_manager.start(sub_block2);

    // start jsonrpc
    let sub_block = pubsub
        .subscribe::<Block>(PUBSUB_BROADCAST_BLOCK.to_owned())
        .unwrap();

    if let Err(e) = components_jsonrpc::listen(jrpc_config, jrpc_state, sub_block) {
        log::error!("Failed to start jrpc server: {}", e);
    };
}

fn handle_init(cfg: &Config, genesis_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let mut r = File::open(genesis_path)?;
    let genesis: Genesis = serde_json::from_reader(&mut r)?;
    log::info!("Genesis data: {:?}", genesis);

    let ctx = Context::new();

    // Init Block db
    let path_block = cfg.data_path_for_block();
    log::info!("Data path for block: {:?}", path_block);
    let block_disk_db = Arc::new(RocksDB::new(path_block.to_str().unwrap())?);
    let block_db = Arc::new(BlockStorage::new(block_disk_db));

    if block_db.get_latest_block(ctx.clone()).wait().is_ok() {
        log::error!("There is already a chain, you should specify a new path");
        return Ok(());
    }

    // Init State db
    let path_state = cfg.data_path_for_state();
    log::info!("Data path for state: {:?}", path_state);
    let state_disk_db = Arc::new(RocksDB::new(path_state.to_str().unwrap())?);
    let state_db = Arc::new(TrieDB::new(state_disk_db));

    let (_, state_root_hash) = EVMExecutor::from_genesis(
        &genesis,
        state_db,
        Arc::new(EVMBlockDataProvider::new(Arc::clone(&block_db))),
    )?;
    log::info!("State root hash: {:?}", state_root_hash);

    let mut block_header = BlockHeader::default();
    block_header.prevhash = Hash::from_hex(&genesis.prevhash)?;
    block_header.timestamp = genesis.timestamp;
    block_header.state_root = state_root_hash;
    block_header.quota_limit = cfg.quota_limit;
    let mut block = Block::default();
    block.hash = block_header.hash();
    block.header = block_header;
    log::info!("init state {:?}", block);
    block_db.insert_block(ctx.clone(), block).wait()?;

    // init proof
    block_db
        .update_latest_proof(ctx.clone(), Proof {
            height: 0,
            round: 0,
            ..Default::default()
        })
        .wait()?;

    Ok(())
}
