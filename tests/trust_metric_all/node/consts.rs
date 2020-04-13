pub const CHAIN_CONFIG_PATH: &str = "devtools/chain/config.toml";
pub const CHAIN_GENESIS_PATH: &str = "devtools/chain/genesis.toml";
pub const CHAIN_ID: &str = "0xb6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036";

// Disable ping
pub const NETWORK_PING_INTERVAL: Option<u64> = Some(99999);
// Enough interval for tests
pub const NETWORK_TRUST_METRIC_INTERVAL: Option<u64> = Some(99);
// Trust metric soft hard ban duration
pub const NETWORK_SOFT_BAND_DURATION: Option<u64> = Some(5);

pub const MEMPOOL_POOL_SIZE: usize = 10;
