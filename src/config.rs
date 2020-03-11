use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use serde_derive::Deserialize;

use core_mempool::{DEFAULT_BROADCAST_TXS_INTERVAL, DEFAULT_BROADCAST_TXS_SIZE};
use protocol::types::Hex;

#[derive(Debug, Deserialize)]
pub struct ConfigGraphQL {
    pub listening_address: SocketAddr,
    pub graphql_uri: String,
    pub graphiql_uri: String,
    #[serde(default)]
    pub workers: usize,
    #[serde(default)]
    pub maxconn: usize,
    #[serde(default)]
    pub max_payload_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct ConfigNetwork {
    pub bootstraps:         Option<Vec<ConfigNetworkBootstrap>>,
    pub listening_address:  SocketAddr,
    pub rpc_timeout:        Option<u64>,
    pub selfcheck_interval: Option<u64>,
    pub send_buffer_size:   Option<usize>,
    pub write_timeout:      Option<u64>,
    pub recv_buffer_size:   Option<usize>,
    pub max_frame_length:   Option<usize>,
    pub max_wait_streams:   Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigNetworkBootstrap {
    pub pubkey:  Hex,
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct ConfigConsensus {
    pub sync_txs_chunk_size: usize,
}

impl Default for ConfigConsensus {
    fn default() -> Self {
        Self {
            sync_txs_chunk_size: 5000,
        }
    }
}

fn default_broadcast_txs_size() -> usize {
    DEFAULT_BROADCAST_TXS_SIZE
}

fn default_broadcast_txs_interval() -> u64 {
    DEFAULT_BROADCAST_TXS_INTERVAL
}

#[derive(Debug, Deserialize)]
pub struct ConfigMempool {
    pub pool_size: u64,

    #[serde(default = "default_broadcast_txs_size")]
    pub broadcast_txs_size: usize,
    #[serde(default = "default_broadcast_txs_interval")]
    pub broadcast_txs_interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfigExecutor {
    pub light: bool,
}

#[derive(Debug, Deserialize)]
pub struct ConfigLogger {
    pub filter: String,
    pub log_to_console: bool,
    pub console_show_file_and_line: bool,
    pub log_to_file: bool,
    pub metrics: bool,
    pub log_path: PathBuf,
    #[serde(default)]
    pub modules_level: HashMap<String, String>,
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
    // crypto
    pub privkey: Hex,
    // db config
    pub data_path: PathBuf,

    pub graphql: ConfigGraphQL,
    pub network: ConfigNetwork,
    pub mempool: ConfigMempool,
    pub executor: ConfigExecutor,
    #[serde(default)]
    pub consensus: ConfigConsensus,
    #[serde(default)]
    pub logger: ConfigLogger,
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
