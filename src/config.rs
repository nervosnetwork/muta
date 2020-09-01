use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use serde_derive::Deserialize;

use core_consensus::{DEFAULT_OVERLORD_GAP, DEFAULT_SYNC_TXS_CHUNK_SIZE};
use core_mempool::{DEFAULT_BROADCAST_TXS_INTERVAL, DEFAULT_BROADCAST_TXS_SIZE};
use protocol::types::Hex;

#[derive(Debug, Deserialize)]
pub struct ConfigGraphQL {
    pub listening_address: SocketAddr,
    pub graphql_uri:       String,
    pub graphiql_uri:      String,
    #[serde(default)]
    pub workers:           usize,
    #[serde(default)]
    pub maxconn:           usize,
    #[serde(default)]
    pub max_payload_size:  usize,
    pub tls:               Option<ConfigGraphQLTLS>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigGraphQLTLS {
    pub private_key_file_path:       PathBuf,
    pub certificate_chain_file_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct ConfigNetwork {
    pub bootstraps:                 Option<Vec<ConfigNetworkBootstrap>>,
    pub allowlist:                  Option<Vec<String>>,
    pub allowlist_only:             Option<bool>,
    pub trust_interval_duration:    Option<u64>,
    pub trust_max_history_duration: Option<u64>,
    pub fatal_ban_duration:         Option<u64>,
    pub soft_ban_duration:          Option<u64>,
    pub max_connected_peers:        Option<usize>,
    pub same_ip_conn_limit:         Option<usize>,
    pub inbound_conn_limit:         Option<usize>,
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

fn default_overlord_gap() -> usize {
    DEFAULT_OVERLORD_GAP
}

fn default_sync_txs_chunk_size() -> usize {
    DEFAULT_SYNC_TXS_CHUNK_SIZE
}

#[derive(Debug, Deserialize)]
pub struct ConfigConsensus {
    #[serde(default = "default_overlord_gap")]
    pub overlord_gap:        usize,
    #[serde(default = "default_sync_txs_chunk_size")]
    pub sync_txs_chunk_size: usize,
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
    pub light:             bool,
    pub triedb_cache_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct ConfigRocksDB {
    pub max_open_files: i32,
}

impl Default for ConfigRocksDB {
    fn default() -> Self {
        Self { max_open_files: 64 }
    }
}

#[derive(Debug, Deserialize)]
pub struct ConfigLogger {
    pub filter:                     String,
    pub log_to_console:             bool,
    pub console_show_file_and_line: bool,
    pub log_to_file:                bool,
    pub metrics:                    bool,
    pub log_path:                   PathBuf,
    pub file_size_limit:            u64,
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
            file_size_limit:            1024 * 1024 * 1024, // GiB
            modules_level:              HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ConfigAPM {
    pub service_name:       String,
    pub tracing_address:    SocketAddr,
    pub tracing_batch_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    // crypto
    pub privkey:   Hex,
    // db config
    pub data_path: PathBuf,

    pub graphql:   ConfigGraphQL,
    pub network:   ConfigNetwork,
    pub mempool:   ConfigMempool,
    pub executor:  ConfigExecutor,
    pub consensus: ConfigConsensus,
    #[serde(default)]
    pub logger:    ConfigLogger,
    #[serde(default)]
    pub rocksdb:   ConfigRocksDB,
    pub apm:       Option<ConfigAPM>,
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

    pub fn data_path_for_txs_wal(&self) -> PathBuf {
        let mut path_state = self.data_path.clone();
        path_state.push("txs_wal");
        path_state
    }
}
