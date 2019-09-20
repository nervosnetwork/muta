mod disc;
mod peer;
mod persist;

use peer::PeerState;
use persist::{NoopPersistence, PeerPersistence, Persistence};

pub use disc::DiscoveryAddrManager;
pub use peer::Peer;

use std::{
    cmp::PartialEq,
    collections::{HashMap, HashSet},
    future::Future,
    hash::{Hash, Hasher},
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    future::TryFutureExt,
    pin_mut,
    stream::Stream,
    task::AtomicWaker,
};
use log::{debug, error};
use parking_lot::RwLock;
use protocol::types::UserAddress;
use rand::seq::IteratorRandom;
use tentacle::{
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
    service::{DialProtocol, TargetSession},
    SessionId,
};

use crate::{
    common::HeartBeat,
    error::NetworkError,
    event::{ConnectionEvent, MultiUsersMessage, PeerManagerEvent},
};

const MAX_RETRY_COUNT: usize = 6;

#[derive(Debug, Clone)]
pub struct UnknownAddr {
    addr:        Multiaddr,
    retry_count: usize,
}

impl UnknownAddr {
    pub fn new(addr: Multiaddr) -> Self {
        UnknownAddr {
            addr,
            retry_count: 0,
        }
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
    addr_pid:   RwLock<HashMap<Multiaddr, PeerId>>,
    user_pid:   RwLock<HashMap<UserAddress, PeerId>>,
    pool:       RwLock<HashMap<PeerId, Peer>>,
}

impl Inner {
    pub fn user_peer(&self, user: &UserAddress) -> Option<Peer> {
        let user_pid = self.user_pid.read();
        let pool = self.pool.read();

        user_pid.get(user).and_then(|pid| pool.get(pid).cloned())
    }

    pub fn add_peer(&self, peer_id: PeerId, pubkey: PublicKey, addr: Multiaddr) {
        let mut pool = self.pool.write();
        let mut addr_pid = self.addr_pid.write();
        let mut user_pid = self.user_pid.write();

        user_pid.insert(Peer::pubkey_to_addr(&pubkey), peer_id.clone());
        addr_pid.insert(addr.clone(), peer_id.clone());

        pool.entry(peer_id.clone())
            .or_insert_with(|| Peer::new(peer_id, pubkey))
            .add_addr(addr)
    }

    pub fn add_peer_addr(&self, peer_id: &PeerId, addr: Multiaddr) {
        let mut pool = self.pool.write();

        if !pool.contains_key(&peer_id) {
            debug_assert!(false, "peer {:?} not found", peer_id);

            error!("network: peer manager: peer {:?} not found", peer_id);
            return;
        }

        pool.entry(peer_id.to_owned())
            .and_modify(|peer| peer.add_addr(addr));
    }

    pub fn remove_peer_addr(&self, peer_id: &PeerId, addr: &Multiaddr) {
        self.addr_pid.write().remove(addr);

        if let Some(peer) = self.pool.write().get_mut(peer_id) {
            peer.remove_addr(addr);
        }
    }

    pub fn try_remove_addr(&self, addr: &Multiaddr) {
        if let Some(pid) = self.addr_pid.read().get(addr) {
            self.remove_peer_addr(&pid.clone(), addr);
        }
    }

    pub fn remove_peer(&self, peer_id: &PeerId) {
        self.connecting.write().remove(peer_id);

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
        self.connect_peer(peer_id);
    }

    pub fn connect_peer(&self, peer_id: PeerId) {
        self.connecting.write().insert(peer_id);
    }

    pub fn disconnect_peer(&self, peer_id: &PeerId) {
        self.connecting.write().remove(peer_id);
    }

    pub fn connecting_count(&self) -> usize {
        self.connecting.read().len()
    }

    pub fn unconnected_addrs(&self, max: usize) -> Vec<Multiaddr> {
        let connecting = self.connecting.read();
        let pool = self.pool.read();
        let mut rng = rand::thread_rng();

        let peers = pool
            .values()
            .filter(|peer| !connecting.contains(peer.pid()) && !peer.retry_ready())
            .choose_multiple(&mut rng, max);

        peers.into_iter().map(Peer::owned_addrs).flatten().collect()
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
            connecting: Default::default(),
            addr_pid:   Default::default(),
            user_pid:   Default::default(),
            pool:       Default::default(),
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
}

pub struct PeerManager {
    // core peer pool
    inner:  Arc<Inner>,
    config: PeerManagerConfig,

    // query purpose
    peer_session: HashMap<PeerId, SessionId>,
    session_peer: HashMap<SessionId, PeerId>,

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

        // Register our self
        inner.register_self(config.our_id.clone(), config.pubkey.clone());

        PeerManager {
            inner,
            config,

            peer_session: Default::default(),
            session_peer: Default::default(),

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

    pub fn bootstrap(&mut self) {
        let peers = self.config.bootstraps.iter();
        let addrs = peers.map(Peer::owned_addrs).flatten().collect();
        debug!("network: peer manager: bootstrap addrs {:?}", addrs);

        self.connect_peers(addrs)
    }

    // TODO: Store protocol in Peer or remove it? Right now, `proto`
    // is ignored in ConnectionService.
    fn connect_peers(&mut self, addrs: Vec<Multiaddr>) {
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

            error!("network: peer manager: fatal!!! session {} no peer id", sid);
            return;
        }

        if let Some(pid) = self.session_peer.get(&sid) {
            self.inner.add_peer_addr(&pid, addr);
        }
    }

    fn attach_peer_session(&mut self, pid: PeerId, sid: SessionId) {
        self.inner.connect_peer(pid.clone());

        self.peer_session.insert(pid.clone(), sid);
        self.session_peer.insert(sid, pid);
    }

    fn detach_peer_session(&mut self, pid: &PeerId) {
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

    fn add_peer(&mut self, pid: PeerId, pubkey: PublicKey, addr: Multiaddr) {
        self.unknown_addrs.remove(&UnknownAddr::new(addr.clone()));
        self.inner.add_peer(pid.clone(), pubkey, addr);
    }

    fn remove_peer(&mut self, pid: &PeerId) {
        self.detach_peer_session(&pid);
        self.inner.remove_peer(&pid);
    }

    fn unconnected_addrs(&self, max: usize) -> Vec<Multiaddr> {
        let mut rng = rand::thread_rng();

        let mut condidates = self.inner.unconnected_addrs(max);
        condidates.extend(self.unknown_addrs.iter().take(max).cloned().map(Into::into));

        condidates.into_iter().choose_multiple(&mut rng, max)
    }

    fn process_event(&mut self, event: PeerManagerEvent) {
        match event {
            PeerManagerEvent::AddPeer { pid, pubkey, addr } => {
                self.add_peer(pid, pubkey, addr);
            }
            PeerManagerEvent::UpdatePeerSession { pid, sid } => {
                if let Some(sid) = sid {
                    self.attach_peer_session(pid, sid);
                } else {
                    self.detach_peer_session(&pid);
                }
            }
            PeerManagerEvent::PeerAlive { pid } => {
                self.inner.reset_retry(&pid);
            }
            PeerManagerEvent::RemovePeer { pid, .. } => {
                self.remove_peer(&pid);
            }
            PeerManagerEvent::RemovePeerBySession { sid, .. } => {
                if let Some(pid) = self.session_peer.get(&sid).cloned() {
                    self.remove_peer(&pid);
                } else {
                    self.detach_session_id(sid);
                }
            }
            PeerManagerEvent::RetryPeerLater { pid, .. } => {
                self.inner.increase_retry_count(&pid);

                // TODO: reduce score base on kind, may be ban this peer for a
                // while
                if self.inner.reach_max_retry(&pid) {
                    self.remove_peer(&pid);
                }

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
            PeerManagerEvent::AddUnknownAddr { addr } => {
                self.unknown_addrs.insert(UnknownAddr::new(addr));
            }
            PeerManagerEvent::AddMultiUnknownAddrs { addrs } => {
                for addr in addrs.into_iter() {
                    self.unknown_addrs.insert(UnknownAddr::new(addr));
                }
            }
            PeerManagerEvent::AddSessionAddr { sid, addr } => {
                self.add_session_addr(sid, addr);
            }
            PeerManagerEvent::AddSessionMultiAddrs { sid, addrs } => {
                for addr in addrs.into_iter() {
                    self.add_session_addr(sid, addr);
                }
            }
            PeerManagerEvent::RemoveAddr { addr, .. } => {
                self.unknown_addrs.remove(&addr.clone().into());
                self.inner.try_remove_addr(&addr);
            }
            PeerManagerEvent::RetryAddrLater { addr, .. } => {
                if let Some(mut addr) = self.unknown_addrs.take(&addr.into()) {
                    addr.increase_retry_count();

                    if !addr.reach_max_retry() {
                        self.unknown_addrs.insert(addr);
                    }
                }
            }
            PeerManagerEvent::AddListenAddr { addr } => {
                self.inner.add_peer_addr(&self.config.our_id, addr);
            }
            PeerManagerEvent::RemoveListenAddr { addr } => {
                self.inner.remove_peer_addr(&self.config.our_id, &addr);
            }
            PeerManagerEvent::RouteMultiUsersMessage { users_msg, miss_tx } => {
                let mut no_peers = vec![];
                let mut connected = vec![];
                let mut unconnected_peers = vec![];

                for user_addr in users_msg.user_addrs.into_iter() {
                    if let Some(peer) = self.inner.user_peer(&user_addr) {
                        if let Some(sid) = self.peer_session.get(&peer.pid()) {
                            connected.push(*sid);
                        } else {
                            unconnected_peers.push(peer);
                        }
                    } else {
                        no_peers.push(user_addr);
                    }
                }

                debug!("network: peer manager: no peers {:?}", no_peers);

                // Send message to connected users
                let tar = TargetSession::Multi(connected);
                let MultiUsersMessage { msg, pri, .. } = users_msg;
                let send_msg = ConnectionEvent::SendMsg { tar, msg, pri };

                if self.conn_tx.unbounded_send(send_msg).is_err() {
                    debug!("network: connection service exit");
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
                    debug!("network: peer manager route multi accounts message dropped")
                }
            }
        }
    }
}

// Persist peers during shutdown
impl Drop for PeerManager {
    fn drop(&mut self) {
        let peer_box = self.inner.package_peers();

        if let Err(err) = self.persistence.save(peer_box) {
            error!("network: peer manager: persistence: {}", err);
        }
    }
}

impl Future for PeerManager {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        debug!("netowrk: peer manager polled");

        self.hb_waker.register(ctx.waker());

        // Spawn heart beat
        if let Some(heart_beat) = self.heart_beat.take() {
            let heart_beat = heart_beat.map_err(|_| {
                error!("network: peer manager: fatal: asystole, fallback to passive mode");
            });

            runtime::spawn(heart_beat);
        }

        // Process manager events
        loop {
            let event_rx = &mut self.as_mut().event_rx;
            pin_mut!(event_rx);

            // service ready in common
            let event = crate::service_ready!("peer manager", event_rx.poll_next(ctx));
            debug!("network: peer manager: event [{}]", event);

            self.process_event(event);
        }

        // Check connecting count
        let connecting_count = self.inner.connecting_count();
        if connecting_count < self.config.max_connections {
            debug!("network: peer manager: connections not fullfill");

            let remain_count = self.config.max_connections - connecting_count;
            let unconnected_addrs = self.unconnected_addrs(remain_count);

            debug!(
                "network: peer manager: {} candidate addrs found",
                unconnected_addrs.len()
            );

            if !unconnected_addrs.is_empty() {
                self.connect_peers(unconnected_addrs);
            }
        }

        Poll::Pending
    }
}
