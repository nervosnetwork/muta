use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::{ok, Future};
use serde_derive::Deserialize;

use components_cita_jsonrpc::{Config as JSONRPCConfig, RpcServer};
use components_database::rocks::RocksDB;
use components_executor::evm::{EVMBlockDataProvider, EVMExecutor};
use components_executor::TrieDB;
use components_transaction_pool::HashTransactionPool;
use core_consensus::{solo_interval, Solo};
use core_crypto::{
    secp256k1::{PrivateKey, Secp256k1},
    CryptoTransform,
};
use core_storage::storage::{BlockStorage, Storage};
use core_types::{Block, BlockHeader, Genesis, Hash};
use logger;

#[derive(Debug, Deserialize)]
struct Config {
    //crypto
    privkey: String,

    // db config
    data_path: PathBuf,

    // transaction pool
    pool_size: u64,
    until_block_limit: u64,
    quota_limit: u64,

    // consensus
    consensus_mode: String,
    consensus_size: u64,
    consensus_interval: u64,
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
    // new db
    let block_db = Arc::new(RocksDB::new(cfg.data_path_for_block().to_str().unwrap()).unwrap());
    let state_db = Arc::new(RocksDB::new(cfg.data_path_for_state().to_str().unwrap()).unwrap());

    // new storage and trie db
    let storage = Arc::new(BlockStorage::new(Arc::clone(&block_db)));
    let trie_db = TrieDB::new(Arc::clone(&state_db));

    // new executor
    let block = storage.get_latest_block().wait().unwrap();
    let executor = Arc::new(
        EVMExecutor::from_existing(
            trie_db,
            Box::new(EVMBlockDataProvider::new(Arc::clone(&storage))),
            &block.header.state_root,
        )
        .unwrap(),
    );

    // new tx pool
    let tx_pool = Arc::new(HashTransactionPool::new(
        Arc::clone(&storage),
        cfg.pool_size as usize,
        cfg.until_block_limit,
        cfg.quota_limit,
    ));

    let privkey = PrivateKey::from_bytes(&hex::decode(cfg.privkey.clone()).unwrap()).unwrap();
    // new solo
    let consensus_solo: Solo<_, _, _, Secp256k1> = Solo::new(
        Arc::clone(&executor),
        Arc::clone(&tx_pool),
        Arc::clone(&storage),
        privkey,
        cfg.consensus_size,
    )
    .unwrap();
    let consensus_solo = Arc::new(consensus_solo);

    // run json rpc
    // NOTE: Bind a variable to aviod "drop".
    let _rpc_server = RpcServer::new(
        JSONRPCConfig::default(),
        Arc::clone(&storage),
        Arc::clone(&executor),
        Arc::clone(&tx_pool),
    )
    .unwrap();

    let consensus_interval = Duration::from_millis(cfg.consensus_interval);

    // start consensus
    tokio::run(ok(()).map(move |_| {
        solo_interval(
            Arc::clone(&consensus_solo),
            Instant::now(),
            consensus_interval,
        )
    }));
}

fn handle_init(cfg: &Config, genesis_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let mut r = File::open(genesis_path)?;
    let genesis: Genesis = serde_json::from_reader(&mut r)?;
    log::info!("Genesis data: {:?}", genesis);

    // Init Block db
    let path_block = cfg.data_path_for_block();
    log::info!("Data path for block: {:?}", path_block);
    let block_disk_db = Arc::new(RocksDB::new(path_block.to_str().unwrap())?);
    let block_db = Arc::new(BlockStorage::new(block_disk_db));

    if block_db.get_latest_block().wait().is_ok() {
        log::error!("There is already a chain, you should specify a new path");
        return Ok(());
    }

    // Init State db
    let path_state = cfg.data_path_for_state();
    log::info!("Data path for state: {:?}", path_state);
    let state_disk_db = Arc::new(RocksDB::new(path_state.to_str().unwrap())?);
    let state_db = TrieDB::new(state_disk_db);

    let (_, state_root_hash) = EVMExecutor::from_genesis(
        &genesis,
        state_db,
        Box::new(EVMBlockDataProvider::new(Arc::clone(&block_db))),
    )?;
    log::info!("State root hash: {:?}", state_root_hash);

    let mut block_header = BlockHeader::default();
    block_header.prevhash = Hash::from_hex(&genesis.prevhash)?;
    block_header.timestamp = genesis.timestamp;
    block_header.state_root = state_root_hash;
    block_header.quota_limit = cfg.quota_limit;
    let mut block = Block::default();
    block.header = block_header;
    block_db.insert_block(&block).wait()?;

    Ok(())
}
