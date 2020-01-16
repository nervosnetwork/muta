use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use serde_derive::Deserialize;

use core_consensus::DurationConfig;
use core_mempool::{DEFAULT_BROADCAST_TXS_INTERVAL, DEFAULT_BROADCAST_TXS_SIZE};

#[derive(Debug, Deserialize)]
pub struct ConfigGraphQL {
    pub listening_address: SocketAddr,
    pub graphql_uri:       String,
    pub graphiql_uri:      String,
}

#[derive(Debug, Deserialize)]
pub struct ConfigNetwork {
    pub bootstraps:        Option<Vec<ConfigNetworkBootstrap>>,
    pub listening_address: SocketAddr,
    pub rpc_timeout:       Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigNetworkBootstrap {
    pub pubkey:  String,
    pub address: SocketAddr,
}

fn default_broadcast_txs_size() -> usize {
    DEFAULT_BROADCAST_TXS_SIZE
}

fn default_broadcast_txs_interval() -> u64 {
    DEFAULT_BROADCAST_TXS_INTERVAL
}

#[derive(Debug, Deserialize)]
pub struct ConfigMempool {
    pub timeout_gap: u64,
    pub pool_size:   u64,

    #[serde(default = "default_broadcast_txs_size")]
    pub broadcast_txs_size:     usize,
    #[serde(default = "default_broadcast_txs_interval")]
    pub broadcast_txs_interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfigConsensus {
    pub cycles_limit:  u64,
    pub cycles_price:  u64,
    pub interval:      u64,
    pub duration:      DurationConfig,
    pub verifier_list: Vec<String>,
    pub public_keys:   Vec<String>,
    pub common_ref:    String,
}

#[derive(Debug, Deserialize)]
pub struct ConfigExecutor {
    pub light: bool,
}

#[derive(Debug, Deserialize)]
pub struct ConfigLogger {
    pub filter:                     String,
    pub log_to_console:             bool,
    pub console_show_file_and_line: bool,
    pub log_to_file:                bool,
    pub metrics:                    bool,
    pub log_path:                   PathBuf,
    #[serde(default)]
    pub modules_level:              HashMap<String, String>,
}

impl Default for ConfigLogger {
    fn default() -> Self {
        Self {
            filter:                     "info".into(),
            log_to_console:             true,
            console_show_file_and_line: false,
            log_to_file:                true,
            metrics:                    true,
            log_path:                   "logs/".into(),
            modules_level:              HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    // chain id
    pub chain_id:  String,
    // crypto
    pub privkey:   String,
    // db config
    pub data_path: PathBuf,

    pub graphql:   ConfigGraphQL,
    pub network:   ConfigNetwork,
    pub mempool:   ConfigMempool,
    pub consensus: ConfigConsensus,
    pub executor:  ConfigExecutor,
    #[serde(default)]
    pub logger:    ConfigLogger,
}

impl Config {
    pub fn data_path_for_state(&self) -> PathBuf {
        let mut path_state = self.data_path.clone();
        path_state.push("rocksdb");
        path_state.push("state_data");
        path_state
    }

    pub fn data_path_for_block(&self) -> PathBuf {
        let mut path_state = self.data_path.clone();
        path_state.push("rocksdb");
        path_state.push("block_data");
        path_state
    }

    // pub fn data_path_for_bft_wal(&self) -> PathBuf {
    //     let mut path_state = self.data_path.clone();
    //     path_state.push("bft_wal");
    //     path_state
    // }
}
