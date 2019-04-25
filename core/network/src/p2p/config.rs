use crate::config::{P2PConfig as RawP2PConfig, LISTENING_ADDRESS};

use tentacle::multiaddr::Multiaddr;
use tentacle::secio::{PeerId, SecioKeyPair};

use std::clone::Clone;
use std::default::Default;
use std::error::Error;
use std::slice::Iter;

#[derive(Clone, Debug)]
pub struct Config {
    key_pair: SecioKeyPair,

    bootstrap_addresses: Vec<Multiaddr>,
    listening_address:   Multiaddr,
}

impl Config {
    pub fn from_raw(config: RawP2PConfig) -> Result<Self, Box<dyn Error>> {
        // None => random generated one
        let key_pair = config.private_key.map_or_else(
            || Ok(SecioKeyPair::secp256k1_generated()),
            SecioKeyPair::secp256k1_raw_key,
        )?;

        // Empty => solo peer mode
        let parse_multiaddr_vec = |addrs: Vec<String>| -> Result<Vec<Multiaddr>, String> {
            addrs
                .iter()
                .map(|addr| addr.parse::<Multiaddr>().map_err(|err| format!("{}", err)))
                .collect::<Result<Vec<Multiaddr>, String>>()
        };
        let bootstrap_addresses = parse_multiaddr_vec(config.bootstrap_addresses)?;

        // None => LISTENING_ADDRESS
        let listening_address = config.listening_address.map_or_else(
            || LISTENING_ADDRESS.parse(),
            |addr| addr.parse::<Multiaddr>(),
        )?;

        let config = Config {
            key_pair,

            bootstrap_addresses,
            listening_address,
        };
        Ok(config)
    }
}

impl Config {
    pub fn peer_id(&self) -> PeerId {
        self.key_pair.to_peer_id()
    }

    pub fn bootstrap_addresses(&self) -> Iter<Multiaddr> {
        self.bootstrap_addresses.iter()
    }

    pub fn listening_address(&self) -> Multiaddr {
        self.listening_address.clone()
    }

    pub fn key_pair(&self) -> SecioKeyPair {
        self.key_pair.clone()
    }
}

impl Default for Config {
    fn default() -> Self {
        let key_pair = SecioKeyPair::secp256k1_generated();

        let listening_address: Multiaddr = LISTENING_ADDRESS.parse().unwrap();
        let bootstrap_addresses = vec![];

        Config {
            key_pair,

            bootstrap_addresses,
            listening_address,
        }
    }
}
