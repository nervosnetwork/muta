mod addr_set;
mod disc;
mod ident;
mod peer;
mod retry;
mod save_restore;
mod shared;
mod time;

use addr_set::PeerAddrSet;
use peer::Peer;
use retry::Retry;
use save_restore::{NoPeerDatFile, PeerDatFile, SaveRestore};

pub use disc::DiscoveryAddrManager;
pub use ident::IdentifyCallback;
pub use peer::{ArcPeer, Connectedness};
pub use shared::{SharedSessions, SharedSessionsConfig};

#[cfg(test)]
mod test_manager;

use std::{
    borrow::Borrow,
    cmp::PartialEq,
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    future::Future,
    hash::{Hash, Hasher},
    iter::FromIterator,
    ops::Deref,
    path::PathBuf,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use derive_more::Display;
use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    pin_mut,
    stream::Stream,
    task::AtomicWaker,
};
use log::{debug, error, info, warn};
use parking_lot::RwLock;
use protocol::types::Address;
use rand::seq::IteratorRandom;
use serde_derive::{Deserialize, Serialize};
#[cfg(not(test))]
use tentacle::context::SessionContext;
use tentacle::{
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
    service::{SessionType, TargetProtocol},
    SessionId,
};

use crate::{
    common::{ConnectedAddr, HeartBeat},
    error::{NetworkError, PeerIdNotFound},
    event::{
        ConnectionErrorKind, ConnectionEvent, ConnectionType, MisbehaviorKind, PeerManagerEvent,
        SessionErrorKind,
    },
    traits::MultiaddrExt,
};

#[cfg(test)]
use crate::test::mock::SessionContext;

const REPEATED_CONNECTION_TIMEOUT: u64 = 30; // seconds
const BACKOFF_BASE: u64 = 2;
const MAX_RETRY_INTERVAL: u64 = 512; // seconds
const MAX_RETRY_COUNT: u8 = 30;
const SHORT_ALIVE_SESSION: u64 = 3; // seconds
const WHITELIST_TIMEOUT: u64 = 60 * 60; // 1 hour
const MAX_CONNECTING_MARGIN: usize = 10;

#[derive(Debug, Clone, Display, Serialize, Deserialize)]
#[display(fmt = "{}", _0)]
pub struct PeerMultiaddr(Multiaddr);

impl PeerMultiaddr {
    pub fn new(mut ma: Multiaddr, peer_id: &PeerId) -> Self {
        if !ma.has_id() {
            ma.push_id(peer_id.to_owned());
        }

        PeerMultiaddr(ma)
    }

    pub fn peer_id(&self) -> PeerId {
        Self::extract_id(&self.0).expect("impossible, should be verified already")
    }

    fn extract_id(ma: &Multiaddr) -> Option<PeerId> {
        if let Some(Ok(peer_id)) = ma
            .id_bytes()
            .map(|bytes| PeerId::from_bytes(bytes.to_vec()))
        {
            Some(peer_id)
        } else {
            None
        }
    }
}

impl Borrow<Multiaddr> for PeerMultiaddr {
    fn borrow(&self) -> &Multiaddr {
        &self.0
    }
}

impl PartialEq for PeerMultiaddr {
    fn eq(&self, other: &PeerMultiaddr) -> bool {
        self.0 == other.0
    }
}

impl Eq for PeerMultiaddr {}

impl Hash for PeerMultiaddr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl Deref for PeerMultiaddr {
    type Target = Multiaddr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Multiaddr> for PeerMultiaddr {
    type Error = PeerIdNotFound;

    fn try_from(ma: Multiaddr) -> Result<PeerMultiaddr, Self::Error> {
        if let Some(_) = Self::extract_id(&ma) {
            Ok(PeerMultiaddr(ma))
        } else {
            Err(PeerIdNotFound(ma))
        }
    }
}

impl Into<Multiaddr> for PeerMultiaddr {
    fn into(self) -> Multiaddr {
        self.0
    }
}

#[derive(Debug)]
struct ConnectingAttempt {
    peer:       ArcPeer,
    multiaddrs: AtomicUsize,
}

impl ConnectingAttempt {
    fn new(peer: ArcPeer) -> Self {
        let multiaddrs = AtomicUsize::new(peer.multiaddrs.connectable_len());

        ConnectingAttempt { peer, multiaddrs }
    }

    fn multiaddrs(&self) -> usize {
        self.multiaddrs.load(Ordering::SeqCst)
    }

    fn complete_one_multiaddr(&self) {
        self.multiaddrs.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Borrow<PeerId> for ConnectingAttempt {
    fn borrow(&self) -> &PeerId {
        &self.peer.id
    }
}

impl PartialEq for ConnectingAttempt {
    fn eq(&self, other: &ConnectingAttempt) -> bool {
        self.peer.id == other.peer.id
    }
}

impl Eq for ConnectingAttempt {}

impl Hash for ConnectingAttempt {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.peer.id.hash(state)
    }
}

#[derive(Debug)]
struct ProtectedPeer {
    chain_addr:    Address,
    authorized_at: AtomicU64,
}

#[derive(Debug, Clone)]
struct ArcProtectedPeer(Arc<ProtectedPeer>);

impl ArcProtectedPeer {
    pub fn new(chain_addr: Address) -> Self {
        let peer = ProtectedPeer {
            chain_addr,
            authorized_at: AtomicU64::new(Self::now()),
        };

        ArcProtectedPeer(Arc::new(peer))
    }

    pub fn refresh_authorized(&self) {
        self.authorized_at.store(Self::now(), Ordering::SeqCst);
    }

    #[cfg(test)]
    pub fn set_authorized_at(&self, at: u64) {
        self.authorized_at.store(at, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub fn authorized_at(&self) -> u64 {
        self.authorized_at.load(Ordering::SeqCst)
    }

    pub fn is_expired(&self) -> bool {
        let expired_at = self
            .authorized_at
            .load(Ordering::SeqCst)
            .saturating_add(WHITELIST_TIMEOUT);

        Self::now() > expired_at
    }

    pub(self) fn now() -> u64 {
        Self::duration_since(SystemTime::now(), UNIX_EPOCH).as_secs()
    }

    fn duration_since(now: SystemTime, early: SystemTime) -> Duration {
        match now.duration_since(early) {
            Ok(duration) => duration,
            Err(e) => e.duration(),
        }
    }
}

impl Borrow<Address> for ArcProtectedPeer {
    fn borrow(&self) -> &Address {
        &self.chain_addr
    }
}

impl PartialEq for ArcProtectedPeer {
    fn eq(&self, other: &ArcProtectedPeer) -> bool {
        self.chain_addr == other.chain_addr
    }
}

impl Eq for ArcProtectedPeer {}

impl Hash for ArcProtectedPeer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.chain_addr.hash(state)
    }
}

impl Deref for ArcProtectedPeer {
    type Target = ProtectedPeer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
struct Session {
    id:             SessionId,
    ctx:            Arc<SessionContext>,
    peer:           ArcPeer,
    blocked:        AtomicBool,
    connected_addr: ConnectedAddr,
}

#[derive(Debug, Clone)]
struct ArcSession(Arc<Session>);

impl ArcSession {
    pub fn new(peer: ArcPeer, ctx: Arc<SessionContext>) -> Self {
        let connected_addr = ConnectedAddr::from(&ctx.address);
        let session = Session {
            id: ctx.id,
            ctx,
            peer,
            blocked: AtomicBool::new(false),
            connected_addr,
        };

        ArcSession(Arc::new(session))
    }

    pub fn block(&self) {
        self.blocked.store(true, Ordering::SeqCst);
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked.load(Ordering::SeqCst)
    }

    pub fn unblock(&self) {
        self.blocked.store(false, Ordering::SeqCst);
    }
}

impl Borrow<SessionId> for ArcSession {
    fn borrow(&self) -> &SessionId {
        &self.id
    }
}

impl PartialEq for ArcSession {
    fn eq(&self, other: &ArcSession) -> bool {
        self.id == other.id
    }
}

impl Eq for ArcSession {}

impl Hash for ArcSession {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl Deref for ArcSession {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct Inner {
    whitelist: RwLock<HashSet<ArcProtectedPeer>>,

    sessions: RwLock<HashSet<ArcSession>>,
    peers:    RwLock<HashSet<ArcPeer>>,
    chain:    RwLock<HashMap<Address, ArcPeer>>,

    listen: RwLock<HashSet<PeerMultiaddr>>,
}

impl Inner {
    pub fn new() -> Self {
        Inner {
            whitelist: Default::default(),

            sessions: Default::default(),
            peers:    Default::default(),
            chain:    Default::default(),

            listen: Default::default(),
        }
    }

    pub fn add_listen(&self, multiaddr: PeerMultiaddr) {
        self.listen.write().insert(multiaddr);
    }

    pub fn listen(&self) -> HashSet<PeerMultiaddr> {
        self.listen.read().clone()
    }

    pub fn remove_listen(&self, multiaddr: &PeerMultiaddr) {
        self.listen.write().remove(multiaddr);
    }

    pub fn connected(&self) -> usize {
        self.sessions.read().len()
    }

    pub fn add_peer(&self, peer: ArcPeer) {
        self.peers.write().insert(peer.clone());
        if let Some(chain_addr) = peer.owned_chain_addr() {
            self.chain.write().insert(chain_addr, peer);
        }
    }

    pub(self) fn restore(&self, peers: Vec<ArcPeer>) {
        let chain_peers: Vec<_> = peers
            .clone()
            .into_iter()
            .filter_map(|p| p.owned_chain_addr().map(|a| (a, p)))
            .collect();

        self.peers.write().extend(peers);
        self.chain.write().extend(chain_peers);
    }

    pub fn peer(&self, peer_id: &PeerId) -> Option<ArcPeer> {
        self.peers.read().get(peer_id).cloned()
    }

    pub fn contains(&self, peer_id: &PeerId) -> bool {
        self.peers.read().contains(peer_id)
    }

    pub fn connectable_peers(&self, max: usize) -> Vec<ArcPeer> {
        let connectable = |p: &'_ &ArcPeer| -> bool {
            (p.connectedness() == Connectedness::NotConnected
                || p.connectedness() == Connectedness::CanConnect)
                && p.retry.ready()
                && p.multiaddrs.connectable_len() > 0
        };

        let mut rng = rand::thread_rng();
        let book = self.peers.read();
        let qualified_peers = book.iter().filter(connectable).map(ArcPeer::to_owned);

        qualified_peers.choose_multiple(&mut rng, max)
    }

    pub fn remove_peer(&self, peer_id: &PeerId) -> Option<ArcPeer> {
        let opt_peer = { self.peers.write().take(peer_id) };
        if let Some(chain_addr) = opt_peer.and_then(|p| p.owned_chain_addr()) {
            self.chain.write().remove(&chain_addr)
        } else {
            None
        }
    }

    pub fn protect_peers_by_chain_addr(&self, chain_addrs: Vec<Address>) {
        let mut new_protected = Vec::new();

        {
            let whitelist = self.whitelist.read();
            for ca in chain_addrs.into_iter() {
                if let Some(peer) = whitelist.get(&ca) {
                    peer.refresh_authorized();
                } else {
                    new_protected.push(ArcProtectedPeer::new(ca))
                }
            }
        }

        self.whitelist.write().extend(new_protected);
    }

    pub fn is_protected_by_chain_addr(&self, chain_addr: &Address) -> bool {
        self.whitelist.read().contains(chain_addr)
    }

    #[cfg(test)]
    pub fn whitelist(&self) -> HashSet<ArcProtectedPeer> {
        self.whitelist.read().iter().cloned().collect()
    }

    pub fn session(&self, sid: SessionId) -> Option<ArcSession> {
        self.sessions.read().get(&sid).cloned()
    }

    #[allow(dead_code)]
    pub fn share_sessions(&self) -> Vec<ArcSession> {
        self.sessions.read().iter().cloned().collect()
    }

    pub fn remove_session(&self, sid: SessionId) -> Option<ArcSession> {
        self.sessions.write().take(&sid)
    }

    pub fn package_peers(&self) -> Vec<ArcPeer> {
        self.peers.read().iter().cloned().collect()
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
    pub bootstraps: Vec<ArcPeer>,

    /// Max connections
    pub max_connections: usize,

    /// Routine job interval
    pub routine_interval: Duration,

    /// Peer dat file path
    pub peer_dat_file: PathBuf,
}

#[derive(Clone)]
pub struct PeerManagerHandle {
    inner: Arc<Inner>,
}

impl PeerManagerHandle {
    pub fn peer_id(&self, sid: SessionId) -> Option<PeerId> {
        self.inner.session(sid).map(|s| s.peer.owned_id())
    }

    pub fn random_addrs(&self, max: usize) -> Vec<Multiaddr> {
        let mut rng = rand::thread_rng();
        let book = self.inner.peers.read();
        let peers = book.iter().choose_multiple(&mut rng, max);

        // Should always include our self
        let our_self = self.listen_addrs();
        let condidates = peers.into_iter().map(|p| p.multiaddrs.all_raw()).flatten();

        our_self.into_iter().chain(condidates).take(max).collect()
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        let listen = self.inner.listen();
        debug_assert!(!listen.is_empty(), "listen should alway be set");

        listen.into_iter().map(Into::into).collect()
    }
}

pub struct PeerManager {
    // core peer pool
    inner:      Arc<Inner>,
    config:     PeerManagerConfig,
    peer_id:    PeerId,
    bootstraps: HashSet<ArcPeer>,

    // peers currently connecting
    connecting: HashSet<ConnectingAttempt>,

    event_rx: UnboundedReceiver<PeerManagerEvent>,
    conn_tx:  UnboundedSender<ConnectionEvent>,

    // heart beat, for current connections check, etc
    heart_beat: Option<HeartBeat>,
    hb_waker:   Arc<AtomicWaker>,

    // save restore
    peer_dat_file: Box<dyn SaveRestore>,
}

impl PeerManager {
    pub fn new(
        config: PeerManagerConfig,
        event_rx: UnboundedReceiver<PeerManagerEvent>,
        conn_tx: UnboundedSender<ConnectionEvent>,
    ) -> Self {
        let peer_id = config.our_id.clone();

        let inner = Arc::new(Inner::new());
        let bootstraps = HashSet::from_iter(config.bootstraps.clone());
        let waker = Arc::new(AtomicWaker::new());
        let heart_beat = HeartBeat::new(Arc::clone(&waker), config.routine_interval);
        let peer_dat_file = Box::new(NoPeerDatFile);

        PeerManager {
            inner,
            config,
            peer_id,
            bootstraps,

            connecting: Default::default(),

            event_rx,
            conn_tx,

            heart_beat: Some(heart_beat),
            hb_waker: waker,

            peer_dat_file,
        }
    }

    pub fn handle(&self) -> PeerManagerHandle {
        PeerManagerHandle {
            inner: Arc::clone(&self.inner),
        }
    }

    #[cfg(test)]
    pub(self) fn inner(&self) -> Arc<Inner> {
        Arc::clone(&self.inner)
    }

    pub fn share_session_book(&self, config: SharedSessionsConfig) -> SharedSessions {
        SharedSessions::new(Arc::clone(&self.inner), config)
    }

    pub fn enable_save_restore(&mut self) {
        let peer_dat_file = PeerDatFile::new(&self.config.peer_dat_file);

        self.peer_dat_file = Box::new(peer_dat_file);
    }

    pub fn restore_peers(&self) -> Result<(), NetworkError> {
        let peers = self.peer_dat_file.restore()?;
        self.inner.restore(peers);
        Ok(())
    }

    pub fn bootstrap(&mut self) {
        // Insert bootstrap peers
        for peer in self.bootstraps.iter() {
            info!("network: {:?}: bootstrap peer: {}", self.peer_id, peer);

            if let Some(peer_exist) = self.inner.peer(&peer.id) {
                info!("restored peer {:?} found, insert multiaddr only", peer.id);
                peer_exist.multiaddrs.insert(peer.multiaddrs.all());
            } else {
                self.inner.add_peer(peer.clone());
            }
        }

        self.connect_peers(self.bootstraps.iter().cloned().collect());
    }

    pub fn disconnect_session(&self, sid: SessionId) {
        let disconnect_peer = ConnectionEvent::Disconnect(sid);
        if self.conn_tx.unbounded_send(disconnect_peer).is_err() {
            error!("network: connection service exit");
        }
    }

    fn new_session(&mut self, pubkey: PublicKey, ctx: Arc<SessionContext>) {
        let remote_peer_id = pubkey.peer_id();
        let remote_multiaddr = PeerMultiaddr::new(ctx.address.to_owned(), &remote_peer_id);

        // Remove from connecting if we dial this peer or create new one
        self.connecting.remove(&remote_peer_id);
        let opt_peer = self.inner.peer(&remote_peer_id);
        let remote_peer = opt_peer.unwrap_or_else(|| ArcPeer::new(remote_peer_id.clone()));

        if !remote_peer.has_pubkey() {
            if let Err(e) = remote_peer.set_pubkey(pubkey.clone()) {
                error!("impossible, set public key failed {}", e);
            }
        }

        // Inbound address is client address, it's useless
        match ctx.ty {
            SessionType::Inbound => remote_peer.multiaddrs.remove(&remote_multiaddr),
            SessionType::Outbound => {
                if remote_peer.multiaddrs.contains(&remote_multiaddr) {
                    remote_peer.multiaddrs.reset_failure(&remote_multiaddr);
                } else {
                    remote_peer.multiaddrs.insert(vec![remote_multiaddr]);
                }
            }
        }

        if self.inner.connected() >= self.config.max_connections {
            let protected = match Peer::pubkey_to_chain_addr(&pubkey) {
                Ok(ca) => self.inner.is_protected_by_chain_addr(&ca),
                _ => false,
            };

            if !protected {
                remote_peer.mark_disconnected();
                self.disconnect_session(ctx.id);
                return;
            }
        }

        // Currently we only save accepted peer.
        // TODO: ban ip for too many different peer id within a short period
        // TODO: save to database
        if !self.inner.contains(&remote_peer_id) {
            self.inner.add_peer(remote_peer.clone());
        }

        let connectedness = remote_peer.connectedness();
        if connectedness == Connectedness::Connected {
            // This should not happen, because of repeated connection event
            error!("got new session event on same peer {:?}", remote_peer.id);

            let exist_sid = remote_peer.session_id();
            if exist_sid != ctx.id && self.inner.session(exist_sid).is_some() {
                // We don't support multiple connections, disconnect new one
                self.disconnect_session(ctx.id);
                return;
            }

            if self.inner.session(exist_sid).is_none() {
                // We keep new session, outdated will be updated after we insert
                // it.
                error!("network: impossible, peer session {} outdated", exist_sid);
            }
        }

        let session = ArcSession::new(remote_peer.clone(), Arc::clone(&ctx));
        info!("new session from {}", session.connected_addr);

        self.inner.sessions.write().insert(session);
        remote_peer.mark_connected(ctx.id);
    }

    fn session_closed(&mut self, sid: SessionId) {
        debug!("session {} closed", sid);

        let session = match self.inner.remove_session(sid) {
            Some(s) => s,
            None => return, /* Session may be removed by other event or rejected
                             * due to max connections before insert */
        };

        info!("session closed {}", session.connected_addr);
        session.peer.mark_disconnected();

        if session.peer.alive() < SHORT_ALIVE_SESSION {
            // NOTE: peer maybe abnormally disconnect from others. When we try
            // to reconnect, other peers may treat this as repeated connection,
            // then disconnect. We have to wait for timeout.
            warn!(
                "increase peer {:?} retry due to repeated short live session",
                session.peer.id
            );

            while session.peer.retry.eta() < REPEATED_CONNECTION_TIMEOUT {
                session.peer.retry.inc();
            }
        }
    }

    fn connect_failed(&mut self, addr: Multiaddr, kind: ConnectionErrorKind) {
        let peer_addr: PeerMultiaddr = match addr.clone().try_into() {
            Ok(pma) => pma,
            Err(e) => {
                // All multiaddrs we dial have peer id included
                error!("unconnectable multiaddr {} without peer id {}", addr, e);
                return;
            }
        };

        let count_multiaddr_failure = |multiaddrs: &PeerAddrSet| {
            if let ConnectionErrorKind::Io(_) | ConnectionErrorKind::DNSResolver(_) = kind {
                multiaddrs.inc_failure(&peer_addr);
            } else {
                warn!("give up {} because {}", peer_addr, kind);
                multiaddrs.give_up(&peer_addr);
            }
        };

        let peer_id = peer_addr.peer_id();
        match self.connecting.take(&peer_id) {
            Some(attempt) => {
                attempt.complete_one_multiaddr();
                count_multiaddr_failure(&attempt.peer.multiaddrs);

                if attempt.multiaddrs() == 0 {
                    // No more connecting multiaddrs from this peer
                    // This means all multiaddrs failure
                    attempt.peer.retry.inc();
                    attempt.peer.set_connectedness(Connectedness::CanConnect);

                    if attempt.peer.retry.run_out() {
                        attempt.peer.set_connectedness(Connectedness::Unconnectable);
                    }
                } else {
                    // Wait for other connecting multiaddrs result
                    self.connecting.insert(attempt);
                }
            }
            None => {
                // Peer is already connected using one of its multiaddrs
                if let Some(peer) = self.inner.peer(&peer_id) {
                    count_multiaddr_failure(&peer.multiaddrs);
                }
            }
        }
    }

    fn session_failed(&self, sid: SessionId, kind: SessionErrorKind) {
        debug!("session {} failed", sid);

        let session = match self.inner.remove_session(sid) {
            Some(s) => s,
            None => return, /* Session may be removed by other event or rejected
                             * due to max connections before insert */
        };
        // Ensure we disconnect this peer
        self.disconnect_session(sid);
        session.peer.mark_disconnected();

        if let SessionErrorKind::Io(_) = kind {
            session.peer.retry.inc();
        } else {
            let pid = &session.peer.id;
            let remote_addr = &session.connected_addr;

            warn!("give up peer {:?} from {} {}", pid, remote_addr, kind);
            session.peer.set_connectedness(Connectedness::Unconnectable);
        }
    }

    fn update_peer_alive(&self, pid: &PeerId) {
        if let Some(peer) = self.inner.peer(pid) {
            peer.retry.reset(); // Just in case
            peer.update_alive();
        }
    }

    // TODO: score system
    fn peer_misbehave(&self, pid: PeerId, kind: MisbehaviorKind) {
        use MisbehaviorKind::*;

        let peer = match self.inner.peer(&pid) {
            Some(p) => p,
            None => {
                error!("misbehave peer {:?} not found", pid);
                return;
            }
        };

        let sid = peer.session_id();
        if sid == SessionId::new(0) {
            error!("misbehave peer with session id 0");
            return;
        }

        self.inner.remove_session(sid);
        peer.mark_disconnected();
        // Ensure we disconnect from this peer
        self.disconnect_session(sid);

        match kind {
            PingTimeout => peer.retry.inc(),
            PingUnexpect | Discovery => peer.set_connectedness(Connectedness::Unconnectable), /* Give up this peer */
        }
    }

    fn session_blocked(&self, ctx: Arc<SessionContext>) {
        warn!(
            "session {} blocked, pending data size {}",
            ctx.id,
            ctx.pending_data_size()
        );

        if let Some(session) = self.inner.session(ctx.id) {
            session.block();
        }
    }

    fn connect_peers_now(&mut self, peers: Vec<ArcPeer>) {
        let peer_addrs = peers.into_iter().map(|peer| {
            let addrs = peer.multiaddrs.all_raw();
            self.connecting.insert(ConnectingAttempt::new(peer));

            addrs
        });

        let addrs = peer_addrs.flatten().collect();
        info!("connect addrs {:?}", addrs);

        let connect_attempt = ConnectionEvent::Connect {
            addrs,
            proto: TargetProtocol::All,
        };

        if self.conn_tx.unbounded_send(connect_attempt).is_err() {
            error!("network: connection service exit");
        }
    }

    fn connect_peers(&mut self, peers: Vec<ArcPeer>) {
        let connectable = |p: ArcPeer| -> Option<ArcPeer> {
            let connectedness = p.connectedness();
            if connectedness != Connectedness::CanConnect
                && connectedness != Connectedness::NotConnected
            {
                debug!("peer {:?} connectedness {}", p.id, connectedness);
                None
            } else {
                Some(p)
            }
        };

        let connectable_peers = peers.into_iter().filter_map(connectable).collect();
        self.connect_peers_now(connectable_peers);
    }

    fn connect_peers_by_id(&mut self, pids: Vec<PeerId>) {
        let peers_to_connect = {
            let book = self.inner.peers.read();
            pids.iter()
                .filter_map(|pid| book.get(pid).cloned())
                .collect()
        };

        self.connect_peers(peers_to_connect);
    }

    fn discover_multiaddr(&mut self, addr: Multiaddr) {
        let peer_addr: PeerMultiaddr = match addr.try_into() {
            Ok(pma) => pma,
            _ => return, // Ignore multiaddr without peer id
        };

        // Ignore our self
        if peer_addr.peer_id() == self.peer_id {
            return;
        }

        let peer_id = peer_addr.peer_id();
        if let Some(peer) = self.inner.peer(&peer_id) {
            peer.multiaddrs.insert(vec![peer_addr]);
        } else {
            let new_peer = ArcPeer::new(peer_addr.peer_id());
            new_peer.multiaddrs.insert(vec![peer_addr]);
        }
    }

    fn dicover_multi_multiaddrs(&mut self, addrs: Vec<Multiaddr>) {
        for addr in addrs.into_iter() {
            self.discover_multiaddr(addr);
        }
    }

    fn identified_addrs(&self, pid: &PeerId, addrs: Vec<Multiaddr>) {
        info!("peer {:?} multi identified addrs {:?}", pid, addrs);

        if let Some(peer) = self.inner.peer(pid) {
            // Make sure all addresses include peer id
            let peer_addrs = addrs
                .into_iter()
                .map(|a| PeerMultiaddr::new(a, pid))
                .collect();

            peer.multiaddrs.insert(peer_addrs);
        }
    }

    fn repeated_connection(&mut self, ty: ConnectionType, sid: SessionId, addr: Multiaddr) {
        info!(
            "repeated session {:?}, ty {}, remote addr {:?}",
            sid, ty, addr
        );

        let session = match self.inner.session(sid) {
            Some(s) => s,
            None => {
                // Impossibl
                error!("repeated connection but session {} not found", sid);
                return;
            }
        };

        let peer_addr = PeerMultiaddr::new(addr, &session.peer.id);

        if ty == ConnectionType::Inbound {
            session.peer.multiaddrs.remove(&peer_addr);
            return;
        }

        // Insert multiaddr
        if session.peer.multiaddrs.contains(&peer_addr) {
            session.peer.multiaddrs.reset_failure(&peer_addr);
        } else {
            session.peer.multiaddrs.insert(vec![peer_addr.clone()]);
        }
    }

    fn process_event(&mut self, event: PeerManagerEvent) {
        match event {
            PeerManagerEvent::ConnectPeersNow { pids } => self.connect_peers_by_id(pids),
            PeerManagerEvent::ConnectFailed { addr, kind } => self.connect_failed(addr, kind),
            PeerManagerEvent::NewSession { pubkey, ctx, .. } => self.new_session(pubkey, ctx),
            // NOTE: Alice may disconnect to Bob, but bob didn't know
            // that, so the next time, Alice try to connect to Bob will
            // cause repeated connection. The only way to fix this right
            // now is wait for time out.
            PeerManagerEvent::RepeatedConnection { ty, sid, addr } => {
                self.repeated_connection(ty, sid, addr)
            }
            PeerManagerEvent::SessionBlocked { ctx, .. } => self.session_blocked(ctx),
            PeerManagerEvent::SessionClosed { sid, .. } => self.session_closed(sid),
            PeerManagerEvent::SessionFailed { sid, kind } => self.session_failed(sid, kind),
            PeerManagerEvent::PeerAlive { pid } => self.update_peer_alive(&pid),
            PeerManagerEvent::Misbehave { pid, kind } => self.peer_misbehave(pid, kind),
            PeerManagerEvent::ProtectPeersByChainAddr { chain_addrs } => {
                self.inner.protect_peers_by_chain_addr(chain_addrs);
            }
            PeerManagerEvent::DiscoverMultiAddrs { addrs } => self.dicover_multi_multiaddrs(addrs),
            PeerManagerEvent::IdentifiedAddrs { pid, addrs } => self.identified_addrs(&pid, addrs),
            PeerManagerEvent::AddNewListenAddr { addr } => {
                let peer_addr = PeerMultiaddr::new(addr, &self.peer_id);
                self.inner.add_listen(peer_addr);
            }
            PeerManagerEvent::RemoveListenAddr { addr } => {
                self.inner
                    .remove_listen(&PeerMultiaddr::new(addr, &self.peer_id));
            }
        }
    }
}

// Save peers during shutdown
impl Drop for PeerManager {
    fn drop(&mut self) {
        let peers = self.inner.package_peers();

        if let Err(err) = self.peer_dat_file.save(peers) {
            error!("network: peer dat file: {}", err);
        }
    }
}

impl Future for PeerManager {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self.hb_waker.register(ctx.waker());

        // Spawn heart beat
        if let Some(heart_beat) = self.heart_beat.take() {
            tokio::spawn(heart_beat);
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
        let connected_count = self.inner.connected();
        let connection_attempts = connected_count + self.connecting.len();
        let max_connection_attempts = self.config.max_connections + MAX_CONNECTING_MARGIN;

        if connected_count < self.config.max_connections
            && connection_attempts < max_connection_attempts
        {
            let remain_count = max_connection_attempts - connection_attempts;
            let connectable_peers = self.inner.connectable_peers(remain_count);
            let candidate_count = connectable_peers.len();

            debug!(
                "network: {:?}: connections not fullfill, {} candidate peers found",
                self.peer_id, candidate_count
            );

            if !connectable_peers.is_empty() {
                self.connect_peers_now(connectable_peers);
            }
        }

        // Clean expired whitelisted peer
        self.inner.whitelist.write().retain(|p| !p.is_expired());

        Poll::Pending
    }
}
