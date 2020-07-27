use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use serde_derive::Deserialize;

use core_mempool::{DEFAULT_BROADCAST_TXS_INTERVAL, DEFAULT_BROADCAST_TXS_SIZE};
use protocol::types::Hex;

#[derive(Debug, Deserialize)]
pub struct ConfigNetwork {
    pub bootstraps:                 Option<Vec<ConfigNetworkBootstrap>>,
    pub whitelist:                  Option<Vec<String>>,
    pub whitelist_peers_only:       Option<bool>,
    pub trust_interval_duration:    Option<u64>,
    pub trust_max_history_duration: Option<u64>,
    pub fatal_ban_duration:         Option<u64>,
    pub soft_ban_duration:          Option<u64>,
    pub max_connected_peers:        Option<usize>,
    pub listening_address:          SocketAddr,
    pub rpc_timeout:                Option<u64>,
    pub selfcheck_interval:         Option<u64>,
    pub send_buffer_size:           Option<usize>,
    pub write_timeout:              Option<u64>,
    pub recv_buffer_size:           Option<usize>,
    pub max_frame_length:           Option<usize>,
    pub max_wait_streams:           Option<usize>,
    pub ping_interval:              Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigNetworkBootstrap {
    pub peer_id: String,
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
    pub broadcast_txs_size:     usize,
    #[serde(default = "default_broadcast_txs_interval")]
    pub broadcast_txs_interval: u64,
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
    // crypto
    pub privkey: Hex,

    pub network:   ConfigNetwork,
    pub mempool:   ConfigMempool,
    pub executor:  ConfigExecutor,
    #[serde(default)]
    pub consensus: ConfigConsensus,
    #[serde(default)]
    pub logger:    ConfigLogger,
}
