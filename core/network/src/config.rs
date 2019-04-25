use serde_derive::Deserialize;

use std::default::Default;

pub const LISTENING_ADDRESS: &str = "/ip4/127.0.0.1/tcp/2077";

/// Configuration for p2p service
#[derive(Debug, Deserialize)]
pub struct P2PConfig {
    /// Encrypt connection and generate peer id. Currently only secp256k1.
    /// note: If not provided, will generate random one for you.
    pub private_key: Option<String>,

    /// Bootstrap peer addresses, see `listening_address`
    /// Default: empty, solo peer mode
    pub bootstrap_addresses: Vec<String>,
    /// Listening address
    /// Default: "/ip4/127.0.0.1/tcp/2077"
    pub listening_address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub p2p: P2PConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            p2p: P2PConfig {
                private_key: None,

                bootstrap_addresses: vec![],
                listening_address:   None,
            },
        }
    }
}
