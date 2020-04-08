pub const CHAIN_CONFIG_PATH: &str = "devtools/chain/config.toml";
pub const CHAIN_GENESIS_PATH: &str = "devtools/chain/genesis.toml";

// Disable ping
pub const NETWORK_PING_INTERVAL: Option<u64> = Some(99999);
// Enough interval for tests
pub const NETWORK_TRUST_METRIC_INTERVAL: Option<u64> = Some(5);
