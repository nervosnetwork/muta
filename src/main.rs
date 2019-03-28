use components_database::rocks::RocksDB;
use components_executor::evm::{EVMBlockDataProvider, EVMExecutor};
use components_executor::TrieDB;
use core_storage::storage::{BlockStorage, Storage};
use core_types::{Block, BlockHeader, Genesis, Hash};
use futures::future::Future;
use log::{error, info};
use logger;
use serde_derive::Deserialize;
use std::error::Error;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct Config {
    data_path: PathBuf,
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

fn handle_init(cfg: &Config, genesis_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let mut r = File::open(genesis_path)?;
    let genesis: Genesis = serde_json::from_reader(&mut r)?;
    info!("Genesis data: {:?}", genesis);

    // Init Block db
    let path_block = cfg.data_path_for_block();
    info!("Data path for block: {:?}", path_block);
    let block_disk_db = Arc::new(RocksDB::new(path_block.to_str().unwrap())?);
    let block_db = Arc::new(BlockStorage::new(block_disk_db));

    if block_db.get_latest_block().wait().is_ok() {
        error!("There is already a chain, you should specify a new path");
        return Ok(());
    }

    // Init State db
    let path_state = cfg.data_path_for_state();
    info!("Data path for state: {:?}", path_state);
    let state_disk_db = Arc::new(RocksDB::new(path_state.to_str().unwrap())?);
    let state_db = TrieDB::new(state_disk_db);

    let (_, state_root_hash) = EVMExecutor::from_genesis(
        &genesis,
        state_db,
        Box::new(EVMBlockDataProvider::new(Arc::clone(&block_db))),
    )?;
    info!("State root hash: {:?}", state_root_hash);

    let mut block_header = BlockHeader::default();
    block_header.prevhash = Hash::from_hex(&genesis.prevhash)?;
    block_header.timestamp = genesis.timestamp;
    block_header.state_root = state_root_hash;
    let mut block = Block::default();
    block.header = block_header;
    block_db.insert_block(&block).wait()?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    logger::init(logger::Flag::Main);
    let matches = clap::App::new("Muta")
        .version("0.1")
        .author("Cryptape Technologies <contact@cryptape.com>")
        .arg(clap::Arg::from_usage(
            "-c --config=[FILE] 'a required file for the configuration'",
        ))
        .subcommand(
            clap::SubCommand::with_name("init")
                .about("Initializes a new genesis block and definition for the network")
                .arg(clap::Arg::from_usage(
                    "<genesis.json> 'expects a genesis file'",
                )),
        )
        .get_matches();

    let args_config = matches.value_of("config").unwrap_or("config.toml");
    let cfg: Config = config_parser::parse(args_config)?;
    info!("Go with config: {:?}", cfg);

    if let Some(matches) = matches.subcommand_matches("init") {
        let genesis_path = matches.value_of("genesis.json").unwrap();
        info!("Genesis path: {}", genesis_path);
        handle_init(&cfg, genesis_path)?;
    }
    Ok(())
}
