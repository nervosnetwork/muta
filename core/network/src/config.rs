use std::{
    default::Default,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use log::error;
use protocol::ProtocolResult;
use tentacle::{
    multiaddr::{multiaddr, Multiaddr, Protocol},
    secio::{PeerId, SecioKeyPair},
};

use crate::{
    common::socket_to_multi_addr,
    connection::ConnectionConfig,
    error::NetworkError,
    peer_manager::{ArcPeer, PeerManagerConfig, SharedSessionsConfig, TrustMetricConfig},
    selfcheck::SelfCheckConfig,
    traits::MultiaddrExt,
    PeerIdExt,
};

// TODO: 0.0.0.0 expose? 127.0.0.1 doesn't work because of tentacle-discovery.
// Default listen address: 0.0.0.0:2337
pub const DEFAULT_LISTEN_IP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
pub const DEFAULT_LISTEN_PORT: u16 = 2337;
// Default max connections
pub const DEFAULT_MAX_CONNECTIONS: usize = 40;
// Default connection stream frame window lenght
pub const DEFAULT_MAX_FRAME_LENGTH: usize = 4 * 1024 * 1024; // 4 Mib
pub const DEFAULT_BUFFER_SIZE: usize = 24 * 1024 * 1024; // same as tentacle

// Default max wait streams for accept
pub const DEFAULT_MAX_WAIT_STREAMS: usize = 256;
// Default write timeout
pub const DEFAULT_WRITE_TIMEOUT: u64 = 10; // seconds

// Default peer trust metric
pub const DEFAULT_PEER_TRUST_INTERVAL_DURATION: Duration = Duration::from_secs(60);
pub const DEFAULT_PEER_TRUST_MAX_HISTORY_DURATION: Duration =
    Duration::from_secs(24 * 60 * 60 * 10); // 10 day
const DEFAULT_PEER_FATAL_BAN_DURATION: Duration = Duration::from_secs(60 * 60); // 1 hour
const DEFAULT_PEER_SOFT_BAN_DURATION: Duration = Duration::from_secs(60 * 10); // 10 minutes

// Default peer data persistent path
pub const DEFAULT_PEER_FILE_NAME: &str = "peers";
pub const DEFAULT_PEER_FILE_EXT: &str = "dat";
pub const DEFAULT_PEER_DAT_FILE: &str = "./peers.dat";

pub const DEFAULT_PING_INTERVAL: u64 = 15;
pub const DEFAULT_PING_TIMEOUT: u64 = 30;
pub const DEFAULT_DISCOVERY_SYNC_INTERVAL: u64 = 60 * 60; // 1 hour

pub const DEFAULT_PEER_MANAGER_HEART_BEAT_INTERVAL: u64 = 30;
pub const DEFAULT_SELF_HEART_BEAT_INTERVAL: u64 = 35;

pub const DEFAULT_RPC_TIMEOUT: u64 = 10;

// Selfcheck
pub const DEFAULT_SELF_CHECK_INTERVAL: u64 = 30;

pub type PrivateKeyHexStr = String;
pub type PeerAddrStr = String;
pub type PeerIdBase58Str = String;

// Example:
//  example.com:2077
struct DnsAddr {
    host: String,
    port: u16,
}

impl FromStr for DnsAddr {
    type Err = NetworkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use NetworkError::UnexpectedPeerAddr;

        let comps = s.split(':').collect::<Vec<_>>();
        if comps.len() != 2 {
            return Err(UnexpectedPeerAddr(s.to_owned()));
        }

        let port = comps[1]
            .parse::<u16>()
            .map_err(|_| UnexpectedPeerAddr(s.to_owned()))?;

        Ok(DnsAddr {
            host: comps[0].to_owned(),
            port,
        })
    }
}

// TODO: support Dns6
impl From<DnsAddr> for Multiaddr {
    fn from(addr: DnsAddr) -> Self {
        multiaddr!(DNS4(&addr.host), TCP(addr.port))
    }
}

#[derive(Debug)]
pub struct NetworkConfig {
    // connection
    pub default_listen:   Multiaddr,
    pub max_connections:  usize,
    pub max_frame_length: usize,
    pub send_buffer_size: usize,
    pub recv_buffer_size: usize,
    pub max_wait_streams: usize,
    pub write_timeout:    u64,

    // peer manager
    pub bootstraps:             Vec<ArcPeer>,
    pub allowlist:              Vec<PeerId>,
    pub allowlist_only:         bool,
    pub enable_save_restore:    bool,
    pub peer_dat_file:          PathBuf,
    pub peer_trust_interval:    Duration,
    pub peer_trust_max_history: Duration,
    pub peer_fatal_ban:         Duration,
    pub peer_soft_ban:          Duration,

    // identity and encryption
    pub secio_keypair: SecioKeyPair,

    // protocol
    pub ping_interval:           Duration,
    pub ping_timeout:            Duration,
    pub discovery_sync_interval: Duration,

    // routine
    pub peer_manager_heart_beat_interval: Duration,
    pub heart_beat_interval:              Duration,

    // rpc
    pub rpc_timeout: Duration,

    // self check
    pub selfcheck_interval: Duration,
}

impl NetworkConfig {
    pub fn new() -> Self {
        let mut listen_addr = Multiaddr::from(DEFAULT_LISTEN_IP_ADDR);
        listen_addr.push(Protocol::TCP(DEFAULT_LISTEN_PORT));

        let peer_manager_hb_interval =
            Duration::from_secs(DEFAULT_PEER_MANAGER_HEART_BEAT_INTERVAL);

        NetworkConfig {
            default_listen:   listen_addr,
            max_connections:  DEFAULT_MAX_CONNECTIONS,
            max_frame_length: DEFAULT_MAX_FRAME_LENGTH,
            send_buffer_size: DEFAULT_BUFFER_SIZE,
            recv_buffer_size: DEFAULT_BUFFER_SIZE,
            max_wait_streams: DEFAULT_MAX_WAIT_STREAMS,
            write_timeout:    DEFAULT_WRITE_TIMEOUT,

            bootstraps:             Default::default(),
            allowlist:              Default::default(),
            allowlist_only:         false,
            enable_save_restore:    false,
            peer_dat_file:          PathBuf::from(DEFAULT_PEER_DAT_FILE.to_owned()),
            peer_trust_interval:    DEFAULT_PEER_TRUST_INTERVAL_DURATION,
            peer_trust_max_history: DEFAULT_PEER_TRUST_MAX_HISTORY_DURATION,
            peer_fatal_ban:         DEFAULT_PEER_FATAL_BAN_DURATION,
            peer_soft_ban:          DEFAULT_PEER_SOFT_BAN_DURATION,

            secio_keypair: SecioKeyPair::secp256k1_generated(),

            ping_interval:           Duration::from_secs(DEFAULT_PING_INTERVAL),
            ping_timeout:            Duration::from_secs(DEFAULT_PING_TIMEOUT),
            discovery_sync_interval: Duration::from_secs(DEFAULT_DISCOVERY_SYNC_INTERVAL),

            peer_manager_heart_beat_interval: peer_manager_hb_interval,
            heart_beat_interval:              Duration::from_secs(DEFAULT_SELF_HEART_BEAT_INTERVAL),

            rpc_timeout: Duration::from_secs(DEFAULT_RPC_TIMEOUT),

            selfcheck_interval: Duration::from_secs(DEFAULT_SELF_CHECK_INTERVAL),
        }
    }

    pub fn max_connections(mut self, max: Option<usize>) -> Self {
        if let Some(max) = max {
            self.max_connections = max;
        }

        self
    }

    pub fn max_frame_length(mut self, max: Option<usize>) -> Self {
        if let Some(max) = max {
            self.max_frame_length = max;
        }

        self
    }

    pub fn send_buffer_size(mut self, size: Option<usize>) -> Self {
        if let Some(size) = size {
            self.send_buffer_size = size;
        }

        self
    }

    pub fn recv_buffer_size(mut self, size: Option<usize>) -> Self {
        if let Some(size) = size {
            self.recv_buffer_size = size;
        }

        self
    }

    pub fn max_wait_streams(mut self, max: Option<usize>) -> Self {
        if let Some(max) = max {
            self.max_wait_streams = max;
        }

        self
    }

    pub fn write_timeout(mut self, timeout: Option<u64>) -> Self {
        if let Some(timeout) = timeout {
            self.write_timeout = timeout;
        }

        self
    }

    pub fn bootstraps(
        mut self,
        pairs: Vec<(PeerIdBase58Str, PeerAddrStr)>,
    ) -> ProtocolResult<Self> {
        let to_peer = |(pid_str, peer_addr): (PeerIdBase58Str, PeerAddrStr)| -> _ {
            let peer_id = PeerId::from_str_ext(&pid_str)?;
            let mut multiaddr = Self::parse_peer_addr(peer_addr)?;

            let peer = ArcPeer::new(peer_id.clone());

            if let Some(id_bytes) = multiaddr.id_bytes() {
                if id_bytes != peer_id.as_bytes() {
                    error!("network: pubkey doesn't match peer id in {}", multiaddr);
                    return Ok(peer);
                }
            }
            if !multiaddr.has_id() {
                multiaddr.push_id(peer_id);
            }

            peer.multiaddrs.insert_raw(multiaddr);
            Ok(peer)
        };

        let bootstrap_peers = pairs
            .into_iter()
            .map(to_peer)
            .collect::<ProtocolResult<Vec<_>>>()?;

        self.bootstraps = bootstrap_peers;
        Ok(self)
    }

    pub fn allowlist<'a, S: AsRef<[String]>>(mut self, peer_id_strs: S) -> ProtocolResult<Self> {
        let peer_ids = {
            let str_iter = peer_id_strs.as_ref().iter();
            let to_peer_ids = str_iter.map(PeerId::from_str_ext);
            to_peer_ids.collect::<Result<Vec<_>, _>>()?
        };

        self.allowlist = peer_ids;
        Ok(self)
    }

    pub fn allowlist_only(mut self, flag: Option<bool>) -> Self {
        if let Some(flag) = flag {
            self.allowlist_only = flag;
        }
        self
    }

    pub fn peer_dat_file<P: AsRef<Path>>(mut self, path: P) -> Self {
        let mut path = path.as_ref().to_owned();
        path.push(DEFAULT_PEER_FILE_NAME);
        path.set_extension(DEFAULT_PEER_FILE_EXT);

        self.peer_dat_file = path;

        self
    }

    pub fn peer_trust_metric(
        mut self,
        interval: Option<u64>,
        max_history: Option<u64>,
    ) -> ProtocolResult<Self> {
        if let Some(interval) = interval {
            self.peer_trust_interval = Duration::from_secs(interval);
        }
        if let Some(max_hist) = max_history {
            self.peer_trust_max_history = Duration::from_secs(max_hist);
        }

        if self.peer_trust_max_history < self.peer_trust_interval * 20 {
            let interval = self.peer_trust_interval.as_secs();
            Err(NetworkError::SmallTrustMaxHistory(interval * 20).into())
        } else {
            Ok(self)
        }
    }

    pub fn peer_fatal_ban(mut self, duration: Option<u64>) -> Self {
        if let Some(duration) = duration {
            self.peer_fatal_ban = Duration::from_secs(duration);
        }

        self
    }

    pub fn peer_soft_ban(mut self, duration: Option<u64>) -> Self {
        if let Some(duration) = duration {
            self.peer_soft_ban = Duration::from_secs(duration);
        }

        self
    }

    pub fn secio_keypair(mut self, sk_hex: PrivateKeyHexStr) -> ProtocolResult<Self> {
        let maybe_skp = hex::decode(sk_hex).map(SecioKeyPair::secp256k1_raw_key);

        if let Ok(Ok(skp)) = maybe_skp {
            self.secio_keypair = skp;

            Ok(self)
        } else {
            Err(NetworkError::InvalidPrivateKey.into())
        }
    }

    pub fn ping_interval(mut self, interval: Option<u64>) -> Self {
        if let Some(interval) = interval {
            self.ping_interval = Duration::from_secs(interval);
        }

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

    pub fn rpc_timeout(mut self, timeout: Option<u64>) -> Self {
        if let Some(timeout) = timeout {
            self.rpc_timeout = Duration::from_secs(timeout);
        }

        self
    }

    pub fn selfcheck_interval(mut self, interval: Option<u64>) -> Self {
        if let Some(interval) = interval {
            self.selfcheck_interval = Duration::from_secs(interval);
        }

        self
    }

    fn parse_peer_addr(addr: PeerAddrStr) -> ProtocolResult<Multiaddr> {
        if let Ok(socket_addr) = addr.parse::<SocketAddr>() {
            Ok(socket_to_multi_addr(socket_addr))
        } else if let Ok(dns_addr) = addr.parse::<DnsAddr>() {
            Ok(Multiaddr::from(dns_addr))
        } else {
            Err(NetworkError::UnexpectedPeerAddr(addr).into())
        }
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
            send_buffer_size: Some(config.send_buffer_size),
            recv_buffer_size: Some(config.recv_buffer_size),
            max_wait_streams: Some(config.max_wait_streams),
            write_timeout:    Some(config.write_timeout),
        }
    }
}

impl From<&NetworkConfig> for PeerManagerConfig {
    fn from(config: &NetworkConfig) -> PeerManagerConfig {
        let peer_trust_config =
            TrustMetricConfig::new(config.peer_trust_interval, config.peer_trust_max_history);

        PeerManagerConfig {
            our_id:            config.secio_keypair.peer_id(),
            pubkey:            config.secio_keypair.public_key(),
            bootstraps:        config.bootstraps.clone(),
            allowlist:         config.allowlist.clone(),
            allowlist_only:    config.allowlist_only,
            peer_trust_config: Arc::new(peer_trust_config),
            peer_fatal_ban:    config.peer_fatal_ban,
            peer_soft_ban:     config.peer_soft_ban,
            max_connections:   config.max_connections,
            routine_interval:  config.peer_manager_heart_beat_interval,
            peer_dat_file:     config.peer_dat_file.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimeoutConfig {
    pub rpc: Duration,
}

impl From<&NetworkConfig> for TimeoutConfig {
    fn from(config: &NetworkConfig) -> TimeoutConfig {
        TimeoutConfig {
            rpc: config.rpc_timeout,
        }
    }
}

impl From<&NetworkConfig> for SelfCheckConfig {
    fn from(config: &NetworkConfig) -> SelfCheckConfig {
        SelfCheckConfig {
            interval: config.selfcheck_interval,
        }
    }
}

// TODO: checkout max_frame_length
impl From<&NetworkConfig> for SharedSessionsConfig {
    fn from(config: &NetworkConfig) -> Self {
        SharedSessionsConfig {
            write_timeout:          config.write_timeout,
            max_stream_window_size: config.max_frame_length,
        }
    }
}
