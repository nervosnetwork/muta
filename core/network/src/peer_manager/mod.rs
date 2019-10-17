mod disc;
mod ident;
mod peer;
mod persist;

use peer::PeerState;
use persist::{NoopPersistence, PeerPersistence, Persistence};

pub use disc::DiscoveryAddrManager;
pub use ident::IdentifyCallback;
pub use peer::Peer;

use std::{
    cmp::PartialEq,
    collections::{HashMap, HashSet},
    future::Future,
    hash::{Hash, Hasher},
    path::PathBuf,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures::{
    channel::{
        mpsc::{UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    future::TryFutureExt,
    pin_mut,
    stream::Stream,
    task::AtomicWaker,
};
use log::{debug, error, info, warn};
use parking_lot::RwLock;
use protocol::types::UserAddress;
use rand::seq::IteratorRandom;
use tentacle::{
    multiaddr::{Multiaddr, Protocol},
    secio::{PeerId, PublicKey},
    service::{DialProtocol, SessionType, TargetSession},
    SessionId,
};

use crate::{
    common::HeartBeat,
    error::NetworkError,
    event::{ConnectionEvent, ConnectionType, MultiUsersMessage, PeerManagerEvent, Session},
};

const MAX_RETRY_COUNT: usize = 6;
const ALIVE_RETRY_INTERVAL: u64 = 3; // seconds

#[derive(Debug, Clone)]
pub struct UnknownAddr {
    addr:        Multiaddr,
    connecting:  Arc<AtomicBool>,
    retry_count: usize,
}

impl UnknownAddr {
    pub fn new(addr: Multiaddr) -> Self {
        UnknownAddr {
            addr,
            connecting: Arc::new(AtomicBool::new(false)),
            retry_count: 0,
        }
    }

    pub fn connecting(&self) -> bool {
        self.connecting.load(Ordering::SeqCst)
    }

    pub fn set_connecting(&self, state: bool) {
        self.connecting.store(state, Ordering::SeqCst);
    }

    pub fn increase_retry_count(&mut self) {
        self.retry_count += 1;

        debug_assert!(self.retry_count < MAX_RETRY_COUNT + 1);
    }

    pub fn reach_max_retry(&self) -> bool {
        MAX_RETRY_COUNT + 1 >= self.retry_count
    }
}

impl Into<Multiaddr> for UnknownAddr {
    fn into(self) -> Multiaddr {
        self.addr
    }
}

impl Into<UnknownAddr> for Multiaddr {
    fn into(self) -> UnknownAddr {
        UnknownAddr::new(self)
    }
}

impl PartialEq for UnknownAddr {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}

impl Hash for UnknownAddr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
    }
}

impl Eq for UnknownAddr {}

// The only purpose we separate Inner from PeerManager is because
// discovery protocol needs directly access to PeerManager.
struct Inner {
    connecting: RwLock<HashSet<PeerId>>,
    connected:  RwLock<HashSet<PeerId>>,

    addr_pid: RwLock<HashMap<Multiaddr, PeerId>>,
    user_pid: RwLock<HashMap<UserAddress, PeerId>>,
    pool:     RwLock<HashMap<PeerId, Peer>>,
    listen:   RwLock<Option<Multiaddr>>,
}

impl Inner {
    pub fn peer_exist(&self, pid: &PeerId) -> bool {
        self.pool.read().contains_key(pid)
    }

    pub fn listen(&self) -> Option<Multiaddr> {
        self.listen.read().clone()
    }

    pub fn user_peer(&self, user: &UserAddress) -> Option<Peer> {
        let user_pid = self.user_pid.read();
        let pool = self.pool.read();

        user_pid.get(user).and_then(|pid| pool.get(pid).cloned())
    }

    pub fn pid_user_addr(&self, pid: &PeerId) -> Option<UserAddress> {
        let pool = self.pool.read();

        pool.get(pid).map(|peer| peer.user_addr().clone())
    }

    pub fn peer_addrs(&self, pid: &PeerId) -> Option<Vec<Multiaddr>> {
        self.pool.read().get(pid).map(Peer::owned_addrs)
    }

    pub fn set_listen(&self, addr: Multiaddr) {
        *self.listen.write() = Some(addr);
    }

    pub fn add_peer(&self, peer: Peer) {
        let mut pool = self.pool.write();
        let mut addr_pid = self.addr_pid.write();
        let mut user_pid = self.user_pid.write();

        user_pid.insert(peer.user_addr().clone(), peer.id().clone());

        for addr in peer.addrs().into_iter() {
            addr_pid.insert(addr.clone(), peer.id().clone());
        }

        pool.insert(peer.id().clone(), peer);
    }

    pub fn add_peer_addr(&self, peer_id: &PeerId, addr: Multiaddr) {
        let mut pool = self.pool.write();

        if !pool.contains_key(&peer_id) {
            debug_assert!(false, "peer {:?} not found", peer_id);

            error!("network: peer {:?} not found", peer_id);
            return;
        }

        self.addr_pid.write().insert(addr.clone(), peer_id.clone());

        pool.entry(peer_id.to_owned())
            .and_modify(|peer| peer.add_addr(addr));
    }

    pub fn remove_peer_addr(&self, peer_id: &PeerId, addr: &Multiaddr) {
        self.addr_pid.write().remove(addr);

        if let Some(peer) = self.pool.write().get_mut(peer_id) {
            peer.remove_addr(addr);
        }
    }

    // /ip4/[ip]/[tcp]/[port]
    // /ip4/[ip]/[tcp]/[port]/[p2p]/[peerid]
    pub fn match_pid(&self, addr: &Multiaddr) -> Option<PeerId> {
        let addr_pid = self.addr_pid.read();

        // exact match
        if let Some(pid) = addr_pid.get(addr) {
            return Some(pid.clone());
        }

        // Try root match
        let comps = addr.iter().collect::<Vec<_>>();
        debug_assert!(
            comps.len() > 1,
            "network: multiaddr should contains at least 2 components",
        );

        if comps.len() < 2 {
            return None;
        }

        let root = Multiaddr::empty()
            .with(comps[0].clone())
            .with(comps[1].clone());

        // Currently support P2P address match
        if let Some(match_pid) = addr_pid.get(&root) {
            return match comps.get(2) {
                Some(Protocol::P2p(addr_pid)) if match_pid.as_bytes() == addr_pid.as_bytes() => {
                    Some(match_pid.clone())
                }
                // Root exact match
                None => Some(match_pid.clone()),
                // Not match means this address is other peer's outdated address
                Some(Protocol::P2p(_)) => None,
                _ => {
                    warn!("network: unsupported multiaddr {}", addr);

                    None
                }
            };
        }

        None
    }

    pub fn try_remove_addr(&self, addr: &Multiaddr) {
        if let Some(pid) = self.addr_pid.read().get(addr) {
            self.remove_peer_addr(&pid.clone(), addr);
        }
    }

    pub fn remove_peer(&self, peer_id: &PeerId) {
        self.connecting.write().remove(peer_id);
        self.connected.write().remove(peer_id);

        let mut addr_pid = self.addr_pid.write();
        if let Some(peer) = self.pool.write().remove(peer_id) {
            self.user_pid.write().remove(peer.user_addr());

            for addr in peer.addrs().into_iter() {
                addr_pid.remove(addr);
            }
        }
    }

    pub fn register_self(&self, peer_id: PeerId, pubkey: PublicKey) {
        let user_addr = Peer::pubkey_to_addr(&pubkey);
        let peer = Peer::new(peer_id.clone(), pubkey);

        self.pool.write().insert(peer_id.clone(), peer);
        self.user_pid.write().insert(user_addr, peer_id.clone());
        self.connected.write().insert(peer_id);
    }

    pub fn peer_connected(&self, peer_id: &PeerId) -> bool {
        self.connected.read().contains(peer_id)
    }

    pub fn connect_peer(&self, peer_id: &PeerId) {
        if self.connected.read().contains(peer_id) {
            warn!("network: peer {:?} already connected", peer_id);
            return;
        }

        self.connecting.write().insert(peer_id.clone());
    }

    pub fn set_connected(&self, peer_id: &PeerId) {
        // Clean outbound connection
        self.connecting.write().remove(peer_id);
        self.connected.write().insert(peer_id.clone());

        let mut pool = self.pool.write();
        if let Some(peer) = pool.get_mut(peer_id) {
            peer.update_connect();
        }
    }

    pub fn disconnect_peer(&self, peer_id: &PeerId) {
        self.connecting.write().remove(peer_id);
        self.connected.write().remove(peer_id);

        let mut pool = self.pool.write();
        if let Some(peer) = pool.get_mut(peer_id) {
            peer.update_disconnect();
            peer.update_alive();
        }
    }

    pub fn peer_alive(&self, peer_id: &PeerId) -> Option<u64> {
        self.pool.read().get(peer_id).map(Peer::alive)
    }

    pub fn connection_count(&self) -> usize {
        self.connected.read().len() + self.connecting.read().len()
    }

    pub fn unconnected_peers(&self, max: usize) -> Vec<PeerId> {
        let connecting = self.connecting.read();
        let connected = self.connected.read();
        let pool = self.pool.read();
        let mut rng = rand::thread_rng();

        let qualified_peers = pool.values().filter(|peer| {
            let pid = peer.id();

            !connected.contains(pid) && !connecting.contains(pid) && peer.retry_ready()
        });

        qualified_peers
            .choose_multiple(&mut rng, max)
            .into_iter()
            .map(Peer::id)
            .cloned()
            .collect()
    }

    pub fn increase_retry_count(&self, peer_id: &PeerId) {
        if let Some(peer) = self.pool.write().get_mut(peer_id) {
            peer.increase_retry();

            debug_assert!(MAX_RETRY_COUNT + 1 >= peer.retry_count())
        }
    }

    pub fn reset_retry(&self, peer_id: &PeerId) {
        if let Some(peer) = self.pool.write().get_mut(peer_id) {
            peer.reset_retry();
        }
    }

    pub fn reach_max_retry(&self, peer_id: &PeerId) -> bool {
        if let Some(peer) = self.pool.read().get(peer_id) {
            MAX_RETRY_COUNT <= peer.retry_count()
        } else {
            true
        }
    }

    pub fn package_peers(&self) -> Vec<(PublicKey, PeerState)> {
        let pool = self.pool.read();
        let mut peer_box = Vec::with_capacity(pool.len());

        for peer in pool.values() {
            let pubkey = peer.pubkey().clone();
            let state = peer.state().clone();

            peer_box.push((pubkey, state))
        }

        peer_box
    }

    pub fn insert_peers(&self, peers: Vec<(PublicKey, PeerState)>) {
        let mut pool = self.pool.write();
        let mut user_pid = self.user_pid.write();
        let mut addr_pid = self.addr_pid.write();

        for (pubkey, state) in peers.into_iter() {
            let peer_id = pubkey.peer_id();
            let user_addr = Peer::pubkey_to_addr(&pubkey);

            user_pid.insert(user_addr, peer_id.clone());

            for addr in state.addrs().into_iter() {
                addr_pid.insert(addr.clone(), peer_id.clone());
            }

            pool.entry(peer_id.clone())
                .or_insert_with(|| Peer::new(peer_id.clone(), pubkey))
                .set_state(state);
        }
    }
}

impl Default for Inner {
    fn default() -> Self {
        Inner {
            connected:  Default::default(),
            connecting: Default::default(),

            addr_pid: Default::default(),
            user_pid: Default::default(),
            pool:     Default::default(),
            listen:   Default::default(),
        }
    }
}

// TODO: Store our secret key?
#[derive(Debug)]
pub struct PeerManagerConfig {
    /// Our Peer ID
    pub our_id: PeerId,

    /// Our public key
    pub pubkey: PublicKey,

    /// Bootstrap peers
    pub bootstraps: Vec<Peer>,

    /// Max connections
    pub max_connections: usize,

    /// Routine job interval
    pub routine_interval: Duration,

    /// Peer persistence path
    pub persistence_path: PathBuf,
}

#[derive(Clone)]
pub struct PeerManagerHandle {
    inner: Arc<Inner>,
}

impl PeerManagerHandle {
    pub fn random_addrs(&self, max: usize) -> Vec<Multiaddr> {
        let mut rng = rand::thread_rng();
        let pool = self.inner.pool.read();
        let peers = pool.values().choose_multiple(&mut rng, max);

        peers.into_iter().map(Peer::owned_addrs).flatten().collect()
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        let listen = self.inner.listen();
        debug_assert!(listen.is_some(), "listen should alway be set");

        listen.map(|addr| vec![addr]).unwrap_or_else(Vec::new)
    }
}

pub struct PeerManager {
    // core peer pool
    inner:   Arc<Inner>,
    config:  PeerManagerConfig,
    peer_id: PeerId,

    // query purpose
    peer_session: HashMap<PeerId, SessionId>,
    session_peer: HashMap<SessionId, PeerId>,
    bootstraps:   HashSet<Multiaddr>,

    // unknown
    unknown_addrs: HashSet<UnknownAddr>,

    event_rx: UnboundedReceiver<PeerManagerEvent>,
    conn_tx:  UnboundedSender<ConnectionEvent>,

    // heart beat, for current connections check, etc
    heart_beat: Option<HeartBeat>,
    hb_waker:   Arc<AtomicWaker>,

    // persistence
    persistence: Box<dyn Persistence>,
}

impl PeerManager {
    pub fn new(
        config: PeerManagerConfig,
        event_rx: UnboundedReceiver<PeerManagerEvent>,
        conn_tx: UnboundedSender<ConnectionEvent>,
    ) -> Self {
        let inner = Arc::new(Inner::default());
        let waker = Arc::new(AtomicWaker::new());
        let heart_beat = HeartBeat::new(Arc::clone(&waker), config.routine_interval);
        let persistence = Box::new(NoopPersistence);
        let peer_id = config.our_id.clone();

        // Register our self
        inner.register_self(config.our_id.clone(), config.pubkey.clone());

        PeerManager {
            inner,
            config,
            peer_id,

            peer_session: Default::default(),
            session_peer: Default::default(),
            bootstraps: Default::default(),

            unknown_addrs: Default::default(),

            event_rx,
            conn_tx,

            heart_beat: Some(heart_beat),
            hb_waker: waker,

            persistence,
        }
    }

    pub fn handle(&self) -> PeerManagerHandle {
        PeerManagerHandle {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn enable_persistence(&mut self) {
        let persistence = PeerPersistence::new(&self.config.persistence_path);

        self.persistence = Box::new(persistence);
    }

    pub fn load_peers(&self) -> Result<(), NetworkError> {
        let peer_box = self.persistence.load()?;
        self.inner.insert_peers(peer_box);

        Ok(())
    }

    pub fn set_listen(&self, addr: Multiaddr) {
        self.inner.set_listen(addr)
    }

    pub fn listen(&self) -> Option<Multiaddr> {
        self.inner.listen()
    }

    pub fn bootstrap(&mut self) {
        let peers = &self.config.bootstraps;

        // Insert bootstrap peers
        for peer in peers.iter() {
            info!("network: {:?}: bootstrap peer: {}", self.peer_id, peer);

            self.inner.add_peer(peer.clone());
            self.inner.connect_peer(peer.id());
        }

        // Collect bootstrap addrs
        let addrs: Vec<Multiaddr> = peers.iter().map(Peer::owned_addrs).flatten().collect();

        // Insert bootstrap addrs, so that we can check if an addr is bootstrap
        for addr in addrs.iter() {
            self.bootstraps.insert(addr.clone());
        }

        // Connect bootstrap addrs
        self.connect_peers(addrs)
    }

    // TODO: Store protocol in Peer or remove it? Right now, `proto`
    // is ignored in ConnectionService.
    fn connect_peers(&mut self, addrs: Vec<Multiaddr>) {
        info!("network: {:?}: connect addrs {:?}", self.peer_id, addrs);

        let connect = ConnectionEvent::Connect {
            addrs,
            proto: DialProtocol::All,
        };

        if self.conn_tx.unbounded_send(connect).is_err() {
            error!("network: connection service offline");
        }
    }

    fn add_session_addr(&mut self, sid: SessionId, addr: Multiaddr) {
        self.unknown_addrs.remove(&addr.clone().into());

        if !self.session_peer.contains_key(&sid) {
            debug_assert!(false, "session {} no peer id", sid);

            error!("network: fatal!!! session {} no peer id", sid);
            return;
        }

        if let Some(pid) = self.session_peer.get(&sid) {
            debug!(
                "network: {:?}: add session addr {} to peer {:?}",
                self.peer_id, addr, pid
            );

            self.inner.add_peer_addr(&pid, addr);
        }
    }

    fn attach_peer_session(&mut self, pubkey: PublicKey, session: Session) {
        let Session { sid, addr, ty } = session;

        let user_addr = Peer::pubkey_to_addr(&pubkey).as_hex();
        let pid = pubkey.peer_id();

        let peer_listen = match ty {
            SessionType::Inbound => {
                // Inbound address is useless, we cannot use it to
                // connect to that peer.
                debug!("network: {:?}: inbound addr {:?}", self.peer_id, addr);

                None
            }
            SessionType::Outbound => Some(addr),
        };

        if !self.inner.peer_exist(&pid) {
            info!(
                "network: {:?}: new peer {:?}, user addr {}, session {:?}",
                self.peer_id, pid, user_addr, sid
            );

            self.add_peer(pubkey, peer_listen);
        } else if let Some(addr) = peer_listen {
            self.inner.add_peer_addr(&pid, addr);
        }

        self.inner.set_connected(&pid);

        self.peer_session.insert(pid.clone(), sid);
        self.session_peer.insert(sid, pid);
    }

    fn detach_peer_session(&mut self, pid: PeerId) {
        let user_addr = self.inner.pid_user_addr(&pid);
        info!("network: detach session user addr {:?}", user_addr);

        self.close_peer_session(&pid);

        if let Some(alive) = self.inner.peer_alive(&pid) {
            if alive < ALIVE_RETRY_INTERVAL {
                warn!(
                    "network: {:?}: peer {:?} connection live too short",
                    self.peer_id, pid
                );

                self.increase_peer_retry(&pid);
            }
        }
    }

    fn close_peer_session(&mut self, pid: &PeerId) {
        self.inner.disconnect_peer(pid);

        if let Some(sid) = self.peer_session.remove(pid) {
            self.session_peer.remove(&sid);
        }
    }

    fn detach_session_id(&mut self, sid: SessionId) {
        if let Some(pid) = self.session_peer.remove(&sid) {
            self.inner.disconnect_peer(&pid);
            self.peer_session.remove(&pid);
        }
    }

    fn add_peer(&mut self, pubkey: PublicKey, addr: Option<Multiaddr>) {
        if let Some(addr) = addr {
            self.unknown_addrs.remove(&UnknownAddr::new(addr.clone()));
            self.inner.add_peer(Peer::from_pair((pubkey, addr)));
        } else {
            let pid = pubkey.peer_id();
            self.inner.add_peer(Peer::new(pid, pubkey));
        }
    }

    fn identify_addr(&mut self, addr: Multiaddr) {
        match self.inner.match_pid(&addr) {
            // Only add identified addr to connected peer, offline peer address
            // maybe outdated
            Some(ref pid) if self.inner.peer_connected(pid) => self.inner.add_peer_addr(&pid, addr),
            // Match an offline peer, noop, peer maybe next connection condidate
            Some(_) => (),
            _ => {
                info!(
                    "network: {:?}: discover unknown addr {}",
                    self.peer_id, addr
                );

                self.unknown_addrs.insert(UnknownAddr::new(addr));
            }
        }
    }

    fn is_bootstrap_peer(&self, pid: &PeerId) -> bool {
        for bootstrap in self.config.bootstraps.iter() {
            if bootstrap.id() == pid {
                return true;
            }
        }

        false
    }

    fn cleanly_remove_peer(&mut self, pid: &PeerId) {
        // Clean up session mapping
        self.close_peer_session(&pid);

        if self.is_bootstrap_peer(pid) {
            error!("network: reset max bootstrap {:?} retry count", pid);

            // Reset bootstrap to reasonable retry count
            self.inner.reset_retry(&pid);
            self.inner.increase_retry_count(&pid);
            self.inner.increase_retry_count(&pid);

            return;
        }

        self.inner.remove_peer(&pid);
    }

    // TODO: reduce score base on kind, may be ban this peer for a
    // while
    fn increase_peer_retry(&mut self, pid: &PeerId) {
        self.inner.increase_retry_count(&pid);

        if self.inner.reach_max_retry(&pid) {
            self.cleanly_remove_peer(&pid);
        }
    }

    fn unconnected_unknowns(&self, max: usize) -> Vec<&UnknownAddr> {
        self.unknown_addrs
            .iter()
            .filter(|unknown| !unknown.connecting())
            .take(max)
            .collect()
    }

    fn unconnected_addrs(&self, max: usize) -> Vec<Multiaddr> {
        let mut condidates = Vec::new();
        let mut remain = max;

        let condidate_pids = self.inner.unconnected_peers(max);
        if !condidate_pids.is_empty() {
            let addrs = condidate_pids.iter().map(|pid| {
                self.inner.connect_peer(pid);
                self.inner.peer_addrs(pid).unwrap_or_else(Vec::new)
            });

            condidates = addrs.flatten().collect();
            remain -= condidate_pids.len();
        }

        let unknown_condidates = self.unconnected_unknowns(remain);
        if !unknown_condidates.is_empty() {
            let addrs = unknown_condidates.into_iter().map(|unknown| {
                unknown.set_connecting(true);
                unknown.addr.clone()
            });

            condidates.extend(addrs);
        }

        condidates
    }

    fn route_multi_users_message(
        &mut self,
        users_msg: MultiUsersMessage,
        miss_tx: oneshot::Sender<Vec<UserAddress>>,
    ) {
        let mut no_peers = vec![];
        let mut connected = vec![];
        let mut unconnected_peers = vec![];

        for user_addr in users_msg.user_addrs.into_iter() {
            if let Some(peer) = self.inner.user_peer(&user_addr) {
                if let Some(sid) = self.peer_session.get(&peer.id()) {
                    connected.push(*sid);
                } else {
                    unconnected_peers.push(peer);
                }
            } else {
                no_peers.push(user_addr);
            }
        }

        if !no_peers.is_empty() {
            warn!("network: no peers {:?}", no_peers);
        }

        // Send message to connected users
        let tar = TargetSession::Multi(connected);
        let MultiUsersMessage { msg, pri, .. } = users_msg;
        let send_msg = ConnectionEvent::SendMsg { tar, msg, pri };

        if self.conn_tx.unbounded_send(send_msg).is_err() {
            error!("network: connection service exit");
        }

        // Try connect to unconnected peers
        let unconnected_addrs = unconnected_peers
            .iter()
            .map(Peer::owned_addrs)
            .flatten()
            .collect::<Vec<_>>();

        self.connect_peers(unconnected_addrs);

        // Report missed user addresses
        let mut missed_accounts = unconnected_peers
            .iter()
            .map(Peer::user_addr)
            .cloned()
            .collect::<Vec<_>>();

        missed_accounts.extend(no_peers);

        if miss_tx.send(missed_accounts).is_err() {
            warn!("network: route multi accounts message dropped")
        }
    }

    fn process_event(&mut self, event: PeerManagerEvent) {
        match event {
            PeerManagerEvent::AttachPeerSession { pubkey, session } => {
                self.attach_peer_session(pubkey, session);
            }
            PeerManagerEvent::DetachPeerSession { pid, .. } => {
                self.detach_peer_session(pid);
            }
            PeerManagerEvent::PeerAlive { pid } => {
                let user_addr = self.inner.pid_user_addr(&pid);

                debug!(
                    "network: {:?}: peer alive, user addr {:?}",
                    self.peer_id, user_addr
                );

                self.inner.reset_retry(&pid);
            }
            PeerManagerEvent::RemovePeer { pid, .. } => {
                info!("network: {:?}: remove peer {:?}", self.peer_id, pid);

                self.cleanly_remove_peer(&pid);
            }
            PeerManagerEvent::RemovePeerBySession { sid, .. } => {
                if let Some(pid) = self.session_peer.get(&sid).cloned() {
                    info!("network: {:?}: remove peer {:?}", self.peer_id, pid);

                    self.cleanly_remove_peer(&pid);
                } else {
                    info!("network: {:?}: disconnect session {}", self.peer_id, sid);

                    self.detach_session_id(sid);
                }
            }
            PeerManagerEvent::RetryPeerLater { pid, .. } => {
                info!(
                    "network: {:?}: reconnect peer later {:?}",
                    self.peer_id, pid
                );

                self.increase_peer_retry(&pid);

                if !self.peer_session.contains_key(&pid) {
                    debug!("network: retry peer {:?} but session id not found", pid);
                    return;
                }

                // Make sure that we disconnect this peer
                if let Some(sid) = self.peer_session.get(&pid) {
                    let disconnect_peer = ConnectionEvent::Disconnect(*sid);

                    // TODO: report upstream or wrap some how
                    if self.conn_tx.unbounded_send(disconnect_peer).is_err() {
                        debug!("network: connection service exit");
                    }
                }
            }
            PeerManagerEvent::DiscoverAddr { addr } => {
                self.identify_addr(addr);
            }
            PeerManagerEvent::DiscoverMultiAddrs { addrs } => {
                for addr in addrs.into_iter() {
                    self.identify_addr(addr);
                }
            }
            PeerManagerEvent::IdentifiedAddrs { pid, addrs } => {
                info!(
                    "network: {:?}: add peer {:?} multi identified addrs {:?}",
                    self.peer_id, pid, addrs
                );

                for addr in addrs.into_iter() {
                    self.inner.add_peer_addr(&pid, addr);
                }
            }
            // NOTE: Alice may disconnect to Bob, but bob didn't know
            // that, so the next time, Alice try to connect to Bob will
            // cause repeated connection. The only way to fix this right
            // now is wait for time out.
            PeerManagerEvent::RepeatedConnection { ty, sid, addr } => {
                let remote_addr = match ty {
                    ConnectionType::Dialer => Some(&addr),
                    ConnectionType::Listen => None,
                };

                info!(
                    "network: {:?}: repeated connection, ty {}, session {:?} remote_addr {:?}",
                    self.peer_id, ty, sid, remote_addr
                );

                // TODO: For ConnectionType::Listen, records repeated count,
                // reduce that peer's score, eventually ban it for a while.
                if ty == ConnectionType::Dialer {
                    self.add_session_addr(sid, addr);
                }
            }
            // TODO: ban unconnectable address for a while instead of repeated
            // connection attempts.
            PeerManagerEvent::UnconnectableAddress { addr, .. } => {
                self.unknown_addrs.remove(&addr.clone().into());

                if self.bootstraps.contains(&addr) {
                    error!("network: unconnectable bootstrap address {}", addr);
                    return;
                }

                self.inner.try_remove_addr(&addr);
            }
            PeerManagerEvent::ReconnectLater { addr, .. } => {
                if let Some(mut unknown) = self.unknown_addrs.take(&addr.clone().into()) {
                    unknown.set_connecting(false);
                    unknown.increase_retry_count();

                    if !unknown.reach_max_retry() {
                        self.unknown_addrs.insert(unknown);
                    }
                }

                if let Some(pid) = self.inner.match_pid(&addr) {
                    // If peer is already connected, don't need to reconnect
                    // this address.
                    if self.inner.peer_connected(&pid) {
                        return;
                    }

                    self.increase_peer_retry(&pid);
                }
            }
            PeerManagerEvent::AddListenAddr { addr } => {
                self.inner.add_peer_addr(&self.config.our_id, addr);
            }
            PeerManagerEvent::RemoveListenAddr { addr } => {
                self.inner.remove_peer_addr(&self.config.our_id, &addr);
            }
            PeerManagerEvent::RouteMultiUsersMessage { users_msg, miss_tx } => {
                self.route_multi_users_message(users_msg, miss_tx);
            }
        }
    }
}

// Persist peers during shutdown
impl Drop for PeerManager {
    fn drop(&mut self) {
        let peer_box = self.inner.package_peers();

        if let Err(err) = self.persistence.save(peer_box) {
            error!("network: persistence: {}", err);
        }
    }
}

impl Future for PeerManager {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self.hb_waker.register(ctx.waker());

        // Spawn heart beat
        if let Some(heart_beat) = self.heart_beat.take() {
            let heart_beat = heart_beat.map_err(|_| {
                error!("network: fatal: asystole, fallback to passive mode");
            });

            runtime::spawn(heart_beat);
        }

        // Process manager events
        loop {
            let event_rx = &mut self.as_mut().event_rx;
            pin_mut!(event_rx);

            // service ready in common
            let event = crate::service_ready!("peer manager", event_rx.poll_next(ctx));

            debug!("network: {:?}: event {}", self.peer_id, event);

            self.process_event(event);
        }

        // Check connecting count
        let connection_count = self.inner.connection_count();
        if connection_count < self.config.max_connections {
            let remain_count = self.config.max_connections - connection_count;
            let unconnected_addrs = self.unconnected_addrs(remain_count);
            let candidate_count = unconnected_addrs.len();

            debug!(
                "network: {:?}: connections not fullfill, {} candidate addrs found",
                self.peer_id, candidate_count
            );

            if !unconnected_addrs.is_empty() {
                self.connect_peers(unconnected_addrs);
            }
        }

        Poll::Pending
    }
}
