use std::{
    default::Default,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

use protocol::ProtocolResult;
use tentacle::{
    bytes::Bytes,
    multiaddr::{Multiaddr, Protocol},
    secio::{PublicKey, SecioKeyPair},
};

use crate::{
    common::socket_to_multi_addr,
    connection::ConnectionConfig,
    error::NetworkError,
    peer_manager::{Peer, PeerManagerConfig},
};

// TODO: 0.0.0.0 expose? 127.0.0.1 doesn't work because of tentacle-discovery.
// Default listen address: 0.0.0.0:2337
pub const DEFAULT_LISTEN_IP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
pub const DEFAULT_LISTEN_PORT: u16 = 2337;
// Default max connections
pub const DEFAULT_MAX_CONNECTIONS: usize = 40;
// Default connection stream frame window lenght
pub const DEFAULT_MAX_FRAME_LENGTH: usize = 4 * 1024 * 1024; // 4 Mib

// Default peer data persistent path
pub const DEFAULT_PEER_FILE_NAME: &str = "peers";
pub const DEFAULT_PEER_FILE_EXT: &str = "dat";
pub const DEFAULT_PEER_PERSISTENCE_PATH: &str = "./peers.dat";

pub const DEFAULT_PING_INTERVAL: u64 = 5;
pub const DEFAULT_PING_TIMEOUT: u64 = 10;
pub const DEFAULT_DISCOVERY_SYNC_INTERVAL: u64 = 3;

pub const DEFAULT_PEER_MANAGER_HEART_BEAT_INTERVAL: u64 = 10;
pub const DEFAULT_SELF_HEART_BEAT_INTERVAL: u64 = 15;

pub type PublicKeyBytes = Bytes;
pub type PrivateKeyBytes = Bytes;

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    // connection
    pub default_listen:   Multiaddr,
    pub max_connections:  usize,
    pub max_frame_length: usize,

    // peer manager
    pub bootstraps:         Vec<Peer>,
    pub enable_persistence: bool,
    pub persistence_path:   PathBuf,

    // identity and encryption
    pub secio_keypair: SecioKeyPair,

    // protocol
    pub ping_interval:           Duration,
    pub ping_timeout:            Duration,
    pub discovery_sync_interval: Duration,

    // routine
    pub peer_manager_heart_beat_interval: Duration,
    pub heart_beat_interval:              Duration,
}

impl NetworkConfig {
    pub fn new() -> Self {
        let mut listen_addr = Multiaddr::from(DEFAULT_LISTEN_IP_ADDR);
        listen_addr.push(Protocol::Tcp(DEFAULT_LISTEN_PORT));

        let peer_manager_hb_interval =
            Duration::from_secs(DEFAULT_PEER_MANAGER_HEART_BEAT_INTERVAL);

        NetworkConfig {
            default_listen:   listen_addr,
            max_connections:  DEFAULT_MAX_CONNECTIONS,
            max_frame_length: DEFAULT_MAX_FRAME_LENGTH,

            bootstraps:         Default::default(),
            enable_persistence: false,
            persistence_path:   PathBuf::from(DEFAULT_PEER_PERSISTENCE_PATH.to_owned()),

            secio_keypair: SecioKeyPair::secp256k1_generated(),

            ping_interval:           Duration::from_secs(DEFAULT_PING_INTERVAL),
            ping_timeout:            Duration::from_secs(DEFAULT_PING_TIMEOUT),
            discovery_sync_interval: Duration::from_secs(DEFAULT_DISCOVERY_SYNC_INTERVAL),

            peer_manager_heart_beat_interval: peer_manager_hb_interval,
            heart_beat_interval:              Duration::from_secs(DEFAULT_SELF_HEART_BEAT_INTERVAL),
        }
    }

    pub fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;

        self
    }

    pub fn max_frame_length(mut self, max: usize) -> Self {
        self.max_frame_length = max;

        self
    }

    pub fn bootstraps(mut self, pairs: Vec<(PublicKeyBytes, SocketAddr)>) -> ProtocolResult<Self> {
        let to_peer = |(pk_bytes, socket_addr): (PublicKeyBytes, SocketAddr)| -> _ {
            let pk = PublicKey::decode(pk_bytes.as_ref())
                .ok_or_else(|| NetworkError::InvalidPublicKey)?;
            let multi_addr = socket_to_multi_addr(socket_addr);

            Ok(Peer::from_pair((pk, multi_addr)))
        };

        let bootstrap_peers = pairs
            .into_iter()
            .map(to_peer)
            .collect::<ProtocolResult<Vec<_>>>()?;

        self.bootstraps = bootstrap_peers;
        Ok(self)
    }

    pub fn persistence_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        let mut path = path.as_ref().to_owned();
        path.push(DEFAULT_PEER_FILE_NAME);
        path.set_extension(DEFAULT_PEER_FILE_EXT);

        self.persistence_path = path;

        self
    }

    pub fn secio_keypair(mut self, sk_bytes: PrivateKeyBytes) -> ProtocolResult<Self> {
        let skp = SecioKeyPair::secp256k1_raw_key(sk_bytes)
            .map_err(|_| NetworkError::InvalidPrivateKey)?;

        self.secio_keypair = skp;
        Ok(self)
    }

    // TODO: #[cfg(test)]
    pub fn skp(mut self, skp: SecioKeyPair) -> Self {
        self.secio_keypair = skp;

        self
    }

    pub fn ping_interval(mut self, interval: u64) -> Self {
        self.ping_interval = Duration::from_secs(interval);

        self
    }

    pub fn ping_timeout(mut self, timeout: u64) -> Self {
        self.ping_timeout = Duration::from_secs(timeout);

        self
    }

    pub fn discovery_sync_interval(mut self, interval: u64) -> Self {
        self.discovery_sync_interval = Duration::from_secs(interval);

        self
    }

    pub fn peer_manager_heart_beat_interval(mut self, interval: u64) -> Self {
        self.peer_manager_heart_beat_interval = Duration::from_secs(interval);

        self
    }

    pub fn heart_beat_interval(mut self, interval: u64) -> Self {
        self.heart_beat_interval = Duration::from_secs(interval);

        self
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig::new()
    }
}

impl From<&NetworkConfig> for ConnectionConfig {
    fn from(config: &NetworkConfig) -> ConnectionConfig {
        ConnectionConfig {
            secio_keypair:    config.secio_keypair.clone(),
            max_frame_length: Some(config.max_frame_length),
        }
    }
}

impl From<&NetworkConfig> for PeerManagerConfig {
    fn from(config: &NetworkConfig) -> PeerManagerConfig {
        PeerManagerConfig {
            our_id:           config.secio_keypair.to_peer_id(),
            pubkey:           config.secio_keypair.to_public_key(),
            bootstraps:       config.bootstraps.clone(),
            max_connections:  config.max_connections,
            routine_interval: config.peer_manager_heart_beat_interval,
            persistence_path: config.persistence_path.clone(),
        }
    }
}
