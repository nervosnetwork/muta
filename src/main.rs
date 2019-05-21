#![feature(async_await, await_macro, futures_api)]

use std::cmp;
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::thread::spawn;

use futures::prelude::{FutureExt, TryFutureExt};
use futures01::future::Future as Future01;

use components_database::rocks::{Config as RocksDBConfig, RocksDB};
use components_executor::evm::{EVMBlockDataProvider, EVMExecutor};
use components_executor::TrieDB;
use components_jsonrpc;
use components_transaction_pool::HashTransactionPool;

use common_logger;
use core_consensus::{Bft, ConsensusStatus, Engine, SynchronizerManager};
use core_context::Context;
use core_crypto::{
    secp256k1::{PrivateKey, Secp256k1},
    Crypto, CryptoTransform,
};
use core_network::{Config as NetConfig, PartialService};
use core_pubsub::{PubSub, PUBSUB_BROADCAST_BLOCK};
use core_runtime::Storage;
use core_storage::BlockStorage;
use core_types::{Address, Block, BlockHeader, Genesis, Hash, Proof};

mod config;
use config::Config;

fn main() {
    common_logger::init(common_logger::Flag::Main);
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
    let cfg: Config = common_config_parser::parse(args_config).unwrap();
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
    let db_cfg = rocksdb_cfg_from(cfg);
    let block_path = cfg.data_path_for_block();
    let state_path = cfg.data_path_for_state();
    let block_db = Arc::new(RocksDB::new(block_path, &db_cfg).unwrap());
    let state_db = Arc::new(RocksDB::new(state_path, &db_cfg).unwrap());

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

    // net network
    let network_config = NetConfig {
        private_key:         cfg.network.private_key.to_owned(),
        bootstrap_addresses: cfg.network.bootstrap_addresses.to_owned(),
        listening_address:   cfg.network.listening_address.to_owned(),
        max_connections:     cfg.network.max_connections.to_owned(),
    };

    let partial_network = PartialService::new(network_config).unwrap();
    let outbound = partial_network.outbound();

    // new tx pool
    let tx_pool = Arc::new(HashTransactionPool::new(
        Arc::clone(&storage),
        Arc::clone(&secp),
        outbound.clone(),
        cfg.txpool.pool_size as usize,
        cfg.txpool.until_block_limit,
        cfg.txpool.quota_limit,
        block.header.height,
    ));

    // run json rpc
    let mut jrpc_config = components_jsonrpc::Config::default();
    jrpc_config.listen = cfg.rpc.address.clone();
    jrpc_config.workers = if cfg.rpc.workers != 0 {
        cfg.rpc.workers as usize
    } else {
        cmp::min(2, num_cpus::get())
    };
    jrpc_config.payload_size = cfg.rpc.payload_size;
    let jrpc_state = components_jsonrpc::AppState::new(
        Arc::clone(&executor),
        Arc::clone(&tx_pool),
        Arc::clone(&storage),
        Arc::clone(&secp),
    );

    // new consensus
    let privkey = PrivateKey::from_bytes(&hex::decode(cfg.privkey.clone()).unwrap()).unwrap();

    let pubkey = secp.get_public_key(&privkey).unwrap();
    let node_address = secp.pubkey_to_address(&pubkey);

    let mut verifier_list = Vec::with_capacity(cfg.consensus.verifier_list.len());
    for address in cfg.consensus.verifier_list.iter() {
        verifier_list.push(Address::from_hex(address).unwrap());
    }

    let proof = storage.get_latest_proof(ctx.clone()).wait().unwrap();
    let status = ConsensusStatus {
        height: block.header.height,
        timestamp: block.header.timestamp,
        block_hash: block.hash.clone(),
        state_root: block.header.state_root.clone(),
        tx_limit: cfg.consensus.tx_limit,
        quota_limit: cfg.txpool.quota_limit,
        interval: cfg.consensus.interval,
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
        outbound.clone(),
        &cfg.data_path_for_bft_wal().to_str().unwrap(),
    )
    .unwrap();
    let consensus = Arc::new(consensus);

    // remain network procedures
    let network = partial_network.build(
        Arc::clone(&tx_pool),
        Arc::clone(&consensus),
        Arc::clone(&storage),
    );
    spawn(move || tokio::run(network.run().unit_error().boxed().compat()));

    // start synchronizer
    let sub_block2 = pubsub
        .subscribe::<Block>(PUBSUB_BROADCAST_BLOCK.to_owned())
        .unwrap();
    let synchronizer_manager = SynchronizerManager::new(
        Arc::new(outbound.clone()),
        Arc::clone(&storage),
        cfg.synchronzer.broadcast_status_interval,
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
    let db_cfg = rocksdb_cfg_from(cfg);

    // Init Block db
    let path_block = cfg.data_path_for_block();
    log::info!("Data path for block: {:?}", path_block);
    let block_disk_db = Arc::new(RocksDB::new(path_block, &db_cfg)?);
    let block_db = Arc::new(BlockStorage::new(block_disk_db));

    if block_db.get_latest_block(ctx.clone()).wait().is_ok() {
        log::error!("There is already a chain, you should specify a new path");
        return Ok(());
    }

    // Init State db
    let path_state = cfg.data_path_for_state();
    log::info!("Data path for state: {:?}", path_state);
    let state_disk_db = Arc::new(RocksDB::new(path_state, &db_cfg)?);
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
    block_header.quota_limit = cfg.txpool.quota_limit;
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

fn rocksdb_cfg_from(cfg: &Config) -> RocksDBConfig {
    RocksDBConfig {
        block_size:                     cfg.rocksdb.block_size,
        block_cache_size:               cfg.rocksdb.block_cache_size,
        max_bytes_for_level_base:       cfg.rocksdb.max_bytes_for_level_base,
        max_bytes_for_level_multiplier: cfg.rocksdb.max_bytes_for_level_multiplier,
        write_buffer_size:              cfg.rocksdb.write_buffer_size,
        target_file_size_base:          cfg.rocksdb.target_file_size_base,
        max_write_buffer_number:        cfg.rocksdb.max_write_buffer_number,
        max_background_compactions:     cfg.rocksdb.max_background_compactions,
        max_background_flushes:         cfg.rocksdb.max_background_flushes,
        increase_parallelism:           cfg.rocksdb.increase_parallelism,
    }
}
