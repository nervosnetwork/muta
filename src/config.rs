use std::net::SocketAddr;
use std::path::PathBuf;

use serde_derive::Deserialize;

use core_consensus::DurationConfig;

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
}

#[derive(Debug, Deserialize)]
pub struct ConfigNetworkBootstrap {
    pub pubkey:  String,
    pub address: SocketAddr,
}

#[derive(Debug, Deserialize)]
pub struct ConfigMempool {
    pub timeout_gap: u64,
    pub pool_size:   u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfigConsensus {
    pub cycles_limit:  u64,
    pub cycles_price:  u64,
    pub interval:      u64,
    pub duration:      DurationConfig,
    pub verifier_list: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigExecutor {
    pub light: bool,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    // chain id
    pub chain_id: String,
    // crypto
    pub privkey: String,
    // db config
    pub data_path: PathBuf,

    pub graphql:   ConfigGraphQL,
    pub network:   ConfigNetwork,
    pub mempool:   ConfigMempool,
    pub consensus: ConfigConsensus,
    pub executor:  ConfigExecutor,
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
