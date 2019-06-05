use std::net::SocketAddr;
use std::path::PathBuf;

use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ConfigRPC {
    pub address:      String,
    pub workers:      u64,
    pub payload_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct ConfigNetwork {
    pub private_key:         Option<String>,
    pub bootstrap_addresses: Vec<SocketAddr>,
    pub listening_address:   SocketAddr,
    pub send_buffer_size:    Option<usize>,
    pub recv_buffer_size:    Option<usize>,
    pub max_connections:     usize,
    pub rpc_timeout:         Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigTxPool {
    pub pool_size:         u64,
    pub until_block_limit: u64,
    pub quota_limit:       u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfigConsensus {
    pub tx_limit:      u64,
    pub interval:      u64,
    pub verifier_list: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigSynchronzer {
    pub broadcast_status_interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfigRocksdb {
    pub block_size:                     Option<usize>,
    pub block_cache_size:               Option<u64>,
    pub max_bytes_for_level_base:       Option<u64>,
    pub max_bytes_for_level_multiplier: Option<f64>,
    pub write_buffer_size:              Option<usize>,
    pub target_file_size_base:          Option<u64>,
    pub max_write_buffer_number:        Option<i32>,
    pub max_background_compactions:     Option<i32>,
    pub max_background_flushes:         Option<i32>,
    pub increase_parallelism:           Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    // crypto
    pub privkey: String,
    // db config
    pub data_path: PathBuf,

    pub rpc:         ConfigRPC,
    pub network:     ConfigNetwork,
    pub txpool:      ConfigTxPool,
    pub consensus:   ConfigConsensus,
    pub synchronzer: ConfigSynchronzer,
    pub rocksdb:     ConfigRocksdb,
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

    pub fn data_path_for_bft_wal(&self) -> PathBuf {
        let mut path_state = self.data_path.clone();
        path_state.push("bft_wal");
        path_state
    }
}
