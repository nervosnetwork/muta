use crate::{
    event::{MisbehaviorKind, PeerManagerEvent},
    peer_manager::PeerManagerHandle,
};

use futures::channel::mpsc::UnboundedSender;
use log::{error, warn};
use tentacle::{
    bytes::{Bytes, BytesMut},
    multiaddr::{Multiaddr, Protocol},
    utils::is_reachable,
    SessionId,
};

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    time::Instant,
};

pub(crate) const DEFAULT_MAX_KNOWN: usize = 5000;

pub enum Misbehavior {
    // Already received GetNodes message
    DuplicateGetNodes,
    // Already received Nodes(announce=false) message
    DuplicateFirstNodes,
    // Nodes message include too many items
    TooManyItems { announce: bool, length: usize },
    // Too many address in one item
    TooManyAddresses(usize),
}

/// Misbehavior report result
pub enum MisbehaveResult {
    /// Continue to run
    #[allow(dead_code)]
    Continue,
    /// Disconnect this peer
    Disconnect,
}

impl MisbehaveResult {
    pub fn is_disconnect(&self) -> bool {
        match self {
            MisbehaveResult::Disconnect => true,
            _ => false,
        }
    }
}

struct AddrReporter {
    inner:    UnboundedSender<PeerManagerEvent>,
    shutdown: bool,
}

impl AddrReporter {
    pub fn new(reporter: UnboundedSender<PeerManagerEvent>) -> Self {
        AddrReporter {
            inner:    reporter,
            shutdown: false,
        }
    }

    // TODO: upstream heart-beat check
    pub fn report(&mut self, event: PeerManagerEvent) {
        if self.shutdown {
            return;
        }

        if self.inner.unbounded_send(event).is_err() {
            error!("network: discovery: peer manager offline");

            self.shutdown = true;
        }
    }
}

pub struct AddressManager {
    peer_mgr: PeerManagerHandle,
    reporter: AddrReporter,
}

// FIXME: Should be peer store?
impl AddressManager {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        let reporter = AddrReporter::new(event_tx);

        AddressManager { peer_mgr, reporter }
    }

    pub fn add_new_addr(&mut self, _sid: SessionId, addr: Multiaddr) {
        let add_addr = PeerManagerEvent::DiscoverMultiAddrs { addrs: vec![addr] };

        self.reporter.report(add_addr);
    }

    pub fn add_new_addrs(&mut self, _sid: SessionId, addrs: Vec<Multiaddr>) {
        let add_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs { addrs };

        self.reporter.report(add_multi_addrs);
    }

    // TODO: reduce peer score based on kind
    pub fn misbehave(&mut self, sid: SessionId, _kind: Misbehavior) -> MisbehaveResult {
        warn!("network: session {} misbehave", sid);

        let pid = match self.peer_mgr.peer_id(sid) {
            Some(id) => id,
            None => {
                error!("network: session {} peer id not found", sid);
                return MisbehaveResult::Disconnect;
            }
        };

        // Right now, we just remove peer
        let kind = MisbehaviorKind::Discovery;
        let peer_misbehave = PeerManagerEvent::Misbehave { pid, kind };

        self.reporter.report(peer_misbehave);
        MisbehaveResult::Disconnect
    }

    pub fn get_random(&mut self, n: usize) -> Vec<Multiaddr> {
        self.peer_mgr.random_addrs(n).into_iter().collect()
    }
}

// bitcoin: bloom.h, bloom.cpp => CRollingBloomFilter
pub struct AddrKnown {
    max_known:  usize,
    addrs:      HashSet<ConnectableAddr>,
    addr_times: HashMap<ConnectableAddr, Instant>,
    time_addrs: BTreeMap<Instant, ConnectableAddr>,
}

impl AddrKnown {
    pub(crate) fn new(max_known: usize) -> AddrKnown {
        AddrKnown {
            max_known,
            addrs: HashSet::default(),
            addr_times: HashMap::default(),
            time_addrs: BTreeMap::default(),
        }
    }

    pub(crate) fn insert(&mut self, key: ConnectableAddr) {
        let now = Instant::now();
        self.addrs.insert(key.clone());
        self.time_addrs.insert(now, key.clone());
        self.addr_times.insert(key, now);

        if self.addrs.len() > self.max_known {
            let first_time = {
                let (first_time, first_key) = self.time_addrs.iter().next().unwrap();
                self.addrs.remove(&first_key);
                self.addr_times.remove(&first_key);
                *first_time
            };
            self.time_addrs.remove(&first_time);
        }
    }

    pub(crate) fn contains(&self, addr: &ConnectableAddr) -> bool {
        self.addrs.contains(addr)
    }

    pub(crate) fn remove<'a>(&mut self, addrs: impl Iterator<Item = &'a ConnectableAddr>) {
        addrs.for_each(|addr| {
            self.addrs.remove(addr);
            if let Some(time) = self.addr_times.remove(addr) {
                self.time_addrs.remove(&time);
            }
        })
    }
}

impl Default for AddrKnown {
    fn default() -> AddrKnown {
        AddrKnown::new(DEFAULT_MAX_KNOWN)
    }
}

#[derive(Clone, Debug, PartialOrd, Ord, Eq, PartialEq, Hash)]
pub struct ConnectableAddr {
    host: Bytes,
    port: u16,
}

impl From<&Multiaddr> for ConnectableAddr {
    fn from(addr: &Multiaddr) -> ConnectableAddr {
        use tentacle::multiaddr::Protocol::*;

        let mut host = None;
        let mut port = 0u16;

        for proto in addr.iter() {
            match proto {
                IP4(_) | IP6(_) | DNS4(_) | DNS6(_) | TLS(_) => {
                    let mut buf = BytesMut::new();
                    proto.write_to_bytes(&mut buf);
                    host = Some(buf.freeze());
                }
                TCP(p) => port = p,
                _ => (),
            }
        }

        let host = host.expect("impossible, unsupported host protocol");

        ConnectableAddr { host, port }
    }
}

impl From<Multiaddr> for ConnectableAddr {
    fn from(addr: Multiaddr) -> ConnectableAddr {
        ConnectableAddr::from(&addr)
    }
}

impl From<SocketAddr> for ConnectableAddr {
    fn from(addr: SocketAddr) -> ConnectableAddr {
        let proto = match addr.ip() {
            IpAddr::V4(ipv4) => Protocol::IP4(ipv4),
            IpAddr::V6(ipv6) => Protocol::IP6(ipv6),
        };

        let mut buf = BytesMut::new();
        proto.write_to_bytes(&mut buf);

        ConnectableAddr {
            host: buf.freeze(),
            port: addr.port(),
        }
    }
}

#[allow(dead_code)]
impl ConnectableAddr {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn is_reachable(&self) -> bool {
        let (proto, _) =
            Protocol::from_bytes(&self.host).expect("impossible invalid host protocol");

        match proto {
            Protocol::IP4(ipv4) => is_reachable(IpAddr::V4(ipv4)),
            Protocol::IP6(ipv6) => is_reachable(IpAddr::V6(ipv6)),
            _ => true,
        }
    }
}
