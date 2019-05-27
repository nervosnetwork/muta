use std::default::Default;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_derive::Deserialize;
use tentacle::{multiaddr::Multiaddr, secio::SecioKeyPair};

use crate::{common::socket_to_multiaddr, Error};

pub const DEFAULT_LISTENING_ADDRESS: &str = "127.0.0.1:1337";
pub const DEFAULT_MAXIMUM_CONNECTIONS: usize = 100;

#[derive(Debug, Deserialize)]
pub struct Config {
    // TODO: split encrypt and identity
    // TODO: deserialize to SecioKeyPair instead of String
    /// Encrypt connection and generate peer id. Currently only support
    /// secp256k1.
    ///
    /// note: If not provided, will generate random one every time.
    pub private_key: Option<String>,

    /// Bootstrap peer addresses
    pub bootstrap_addresses: Vec<SocketAddr>,

    /// Listening address
    pub listening_address: SocketAddr,

    /// Send buffer size, default: 1 MiB
    pub send_buffer_size: Option<usize>,

    /// Recv buffer sizse, default: 1 MiB
    pub recv_buffer_size: Option<usize>,

    /// Maximum connected addresses
    pub max_connections: usize,
}

impl Default for Config {
    fn default() -> Self {
        let ip_addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let listening_address = SocketAddr::new(ip_addr, 1337);

        Config {
            private_key: None,
            bootstrap_addresses: vec![],
            listening_address,
            send_buffer_size: None,
            recv_buffer_size: None,
            max_connections: DEFAULT_MAXIMUM_CONNECTIONS,
        }
    }
}

pub struct ConnectionPoolConfig {
    pub key_pair:            SecioKeyPair,
    pub bootstrap_addresses: Vec<Multiaddr>,
    pub listening_address:   Multiaddr,
    pub send_buffer_size:    Option<usize>,
    pub recv_buffer_size:    Option<usize>,
}

impl ConnectionPoolConfig {
    pub fn from_config(config: &Config) -> Result<Self, Error> {
        let key_pair = config.private_key.to_owned().map_or_else(
            || Ok(SecioKeyPair::secp256k1_generated()),
            |key| SecioKeyPair::secp256k1_raw_key(key).map_err(|_| Error::InvalidPrivateKey),
        )?;

        let bootstrap_addresses = config
            .bootstrap_addresses
            .iter()
            .map(socket_to_multiaddr)
            .collect::<Vec<_>>();

        let listening_address = socket_to_multiaddr(&config.listening_address);

        Ok(ConnectionPoolConfig {
            key_pair,
            bootstrap_addresses,
            listening_address,
            send_buffer_size: config.send_buffer_size,
            recv_buffer_size: config.recv_buffer_size,
        })
    }
}
