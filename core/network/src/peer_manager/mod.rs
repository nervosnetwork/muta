mod addr_info;
mod disc;
mod ident;
mod peer;
mod save_restore;
mod shared;

use addr_info::AddrInfo;
use peer::Peer;
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
    collections::HashSet,
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
    event::{ConnectionEvent, ConnectionType, PeerManagerEvent},
    traits::MultiaddrExt,
};

#[cfg(test)]
use crate::test::mock::SessionContext;

macro_rules! peer_id_from_multiaddr {
    ($multiaddr:expr) => {
        $multiaddr
            .id_bytes()
            .map(|bs| PeerId::from_bytes(bs.to_vec()))
    };
}

const MAX_RETRY_COUNT: u8 = 30;
const ALIVE_RETRY_INTERVAL: u64 = 3; // seconds
const WHITELIST_TIMEOUT: u64 = 60 * 60; // 1 hour

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
        if !ma.has_id() {
            Err(PeerIdNotFound(ma))
        } else {
            Ok(PeerMultiaddr(ma))
        }
    }
}

impl Into<Multiaddr> for PeerMultiaddr {
    fn into(self) -> Multiaddr {
        self.0
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

#[derive(Debug, Clone)]
struct ArcPeerByChain(ArcPeer);

impl Borrow<Address> for ArcPeerByChain {
    fn borrow(&self) -> &Address {
        &self.chain_addr
    }
}

impl PartialEq for ArcPeerByChain {
    fn eq(&self, other: &ArcPeerByChain) -> bool {
        self.chain_addr == other.chain_addr
    }
}

impl Eq for ArcPeerByChain {}

impl Hash for ArcPeerByChain {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.chain_addr.hash(state)
    }
}

impl Deref for ArcPeerByChain {
    type Target = ArcPeer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct Inner {
    connection_count: AtomicUsize,

    whitelist: RwLock<HashSet<ArcProtectedPeer>>,
    sessions:  RwLock<HashSet<ArcSession>>,
    peers:     RwLock<HashSet<ArcPeer>>,
    chain:     RwLock<HashSet<ArcPeerByChain>>,

    listen: RwLock<HashSet<PeerMultiaddr>>,
}

impl Inner {
    pub fn new() -> Self {
        Inner {
            connection_count: AtomicUsize::new(0),

            whitelist: Default::default(),
            sessions:  Default::default(),
            peers:     Default::default(),
            chain:     Default::default(),

            listen: Default::default(),
        }
    }

    pub fn add_listen(&self, multiaddr: PeerMultiaddr) {
        self.listen.write().insert(multiaddr);
    }

    pub fn listen_contains(&self, multiaddr: &PeerMultiaddr) -> bool {
        self.listen.read().contains(multiaddr)
    }

    pub fn listen(&self) -> HashSet<PeerMultiaddr> {
        self.listen.read().clone()
    }

    pub fn remove_listen(&self, multiaddr: &PeerMultiaddr) {
        self.listen.write().remove(multiaddr);
    }

    pub fn inc_conn_count(&self) {
        self.connection_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn inc_conn_count_by(&self, n: usize) {
        self.connection_count.fetch_add(n, Ordering::SeqCst);
    }

    pub fn dec_conn_count(&self) {
        self.connection_count.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn conn_count(&self) -> usize {
        self.connection_count.load(Ordering::SeqCst)
    }

    pub fn add_peer(&self, peer: ArcPeer) {
        self.peers.write().insert(peer.clone());
        self.chain.write().insert(ArcPeerByChain(peer));
    }

    pub(self) fn restore(&self, peers: Vec<ArcPeer>) {
        let chain_peers: Vec<_> = peers.clone().into_iter().map(ArcPeerByChain).collect();

        self.peers.write().extend(peers);
        self.chain.write().extend(chain_peers);
    }

    pub fn peer(&self, peer_id: &PeerId) -> Option<ArcPeer> {
        self.peers.read().get(peer_id).cloned()
    }

    pub fn unconnected_peers(&self, max: usize) -> Vec<ArcPeer> {
        let mut rng = rand::thread_rng();
        let book = self.peers.read();
        let qualified_peers = book
            .iter()
            .filter(|p| {
                p.connectedness() == Connectedness::CanConnect
                    && p.retry_ready()
                    && p.multiaddrs_len() > 0
            })
            .map(|p| p.to_owned());

        qualified_peers.choose_multiple(&mut rng, max)
    }

    pub fn remove_peer(&self, peer_id: &PeerId) -> Option<ArcPeer> {
        let opt_peer = { self.peers.write().take(peer_id) };
        if let Some(peer) = opt_peer {
            self.chain.write().take(&*peer.chain_addr).map(|cp| cp.0)
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
    pub fn random_addrs(&self, max: usize) -> Vec<Multiaddr> {
        let mut rng = rand::thread_rng();
        let book = self.inner.peers.read();
        let peers = book.iter().choose_multiple(&mut rng, max);

        // Should always include our self
        let our_self = self.listen_addrs();
        let condidates = peers.into_iter().map(|p| p.raw_multiaddrs()).flatten();

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

    // unknown
    unknown_addrs: HashSet<AddrInfo>,

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

            unknown_addrs: Default::default(),

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
        let peers = &self.config.bootstraps;

        // Insert bootstrap peers
        for peer in peers.iter() {
            info!("network: {:?}: bootstrap peer: {}", self.peer_id, peer);

            if let Some(peer_exist) = self.inner.peer(&peer.id) {
                info!(
                    "network: {:?}: restored peer found, add multiaddr only",
                    self.peer_id
                );
                peer_exist.add_multiaddrs(peer.multiaddrs());
            } else {
                self.inner.add_peer(peer.clone());
            }
        }

        self.connect_peers_now(peers.clone());
    }

    pub fn disconnect_session(&self, sid: SessionId) {
        let disconnect_peer = ConnectionEvent::Disconnect(sid);
        if self.conn_tx.unbounded_send(disconnect_peer).is_err() {
            error!("network: connection service exit");
        }
    }

    pub fn connected_addrs(&self) -> Vec<ConnectedAddr> {
        let sessions = self.inner.share_sessions();

        sessions
            .into_iter()
            .map(|s| s.connected_addr.to_owned())
            .collect()
    }

    pub fn unconnected_addrs(&self, max: usize) -> Vec<Multiaddr> {
        let mut condidates = Vec::new();
        let mut remain = max;

        let condidate_peers = self.inner.unconnected_peers(remain);
        if !condidate_peers.is_empty() {
            self.inner.inc_conn_count_by(condidate_peers.len());

            let addrs = condidate_peers.iter().map(|p| {
                p.set_connectedness(Connectedness::Connecting);
                if p.multiaddrs_len() == 0 {
                    error!("network: unconnected peer has no multiaddr");
                }
                p.raw_multiaddrs()
            });

            condidates = addrs.flatten().collect();
            remain -= condidate_peers.len()
        }

        let unknown_condidates = self.unconnected_unknowns(remain);
        if !unknown_condidates.is_empty() {
            let addrs = unknown_condidates.into_iter().map(|unknown| {
                unknown.mark_connecting();
                unknown.clone().owned_addr()
            });

            condidates.extend(addrs);
        }

        condidates
    }

    fn unconnected_unknowns(&self, max: usize) -> Vec<&AddrInfo> {
        self.unknown_addrs
            .iter()
            .filter(|unknown| !unknown.is_connecting() && unknown.retry_ready())
            .take(max)
            .collect()
    }

    fn new_session(&mut self, pubkey: PublicKey, ctx: Arc<SessionContext>) {
        let remote_multiaddr = PeerMultiaddr::new(ctx.address.to_owned(), &pubkey.peer_id());

        if ctx.ty == SessionType::Inbound {
            // Inbound multiaddrs are useless, always remove them
            self.unknown_addrs.remove(&remote_multiaddr);
        }

        if self.inner.conn_count() >= self.config.max_connections {
            let protected = match Peer::pubkey_to_chain_addr(&pubkey) {
                Ok(ca) => self.inner.is_protected_by_chain_addr(&ca),
                _ => false,
            };

            if !protected {
                self.disconnect_session(ctx.id);
                return;
            }
        }

        let remote_peer_id = pubkey.peer_id();
        let peer = match self.inner.peer(&remote_peer_id) {
            Some(p) => p,
            None => {
                let peer = match ArcPeer::from_pubkey(pubkey) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("network: {}", e);
                        return;
                    }
                };

                self.inner.add_peer(peer.clone());
                peer
            }
        };

        // peer and ctx are moved into closure
        let insert_outbound_multiaddr =
            |self_id: &PeerId, unknown_addrs: &mut HashSet<AddrInfo>| {
                match ctx.ty {
                    SessionType::Inbound => {
                        // We can't use inbound address to connect to this peer
                        debug!("network: {:?}: inbound {:?}", self_id, ctx.address);
                    }
                    SessionType::Outbound => {
                        unknown_addrs.remove(&remote_multiaddr);
                        peer.add_multiaddrs(vec![remote_multiaddr]);
                    }
                }
            };

        let connectedness = peer.connectedness();
        if connectedness == Connectedness::Connected {
            // This should not happen, because of repeated connection event
            error!("network: got new session event on same peer {:?}", peer.id);

            let exist_sid = peer.session_id();
            if exist_sid != ctx.id && self.inner.session(exist_sid).is_some() {
                // We don't support multiple connections, disconnect new one
                self.disconnect_session(ctx.id);
                insert_outbound_multiaddr(&self.peer_id, &mut self.unknown_addrs);
                return;
            }

            if self.inner.session(exist_sid).is_none() {
                // We keep new session, outdated will be updated after we insert
                // it.
                error!("network: bug peer session {} outdated", exist_sid);
            }
        }

        // Connecting/Connected was already counted
        if connectedness != Connectedness::Connecting && connectedness != Connectedness::Connected {
            self.inner.inc_conn_count();
        }

        let session = ArcSession::new(peer.clone(), Arc::clone(&ctx));
        self.inner.sessions.write().insert(session);
        peer.mark_connected(ctx.id);

        // Insert peer multiaddr
        insert_outbound_multiaddr(&self.peer_id, &mut self.unknown_addrs);
    }

    fn session_closed(&mut self, sid: SessionId) {
        info!("network: session {} closed", sid);

        let session = match self.inner.remove_session(sid) {
            Some(s) => s,
            None => return, // Session may be removed by other event
        };

        self.inner.dec_conn_count();
        session.peer.mark_disconnected();

        if session.peer.alive() < ALIVE_RETRY_INTERVAL {
            debug!(
                "network: {:?}: peer {:?} short live session",
                self.peer_id, session.peer.id
            );

            session.peer.increase_retry();
        }
    }

    fn update_peer_alive(&self, pid: &PeerId) {
        if let Some(peer) = self.inner.peer(pid) {
            // Just in cast
            peer.reset_retry();
            peer.update_alive();
        }
    }

    fn remove_peer_by_session(&self, sid: SessionId) {
        let session = match self.inner.remove_session(sid) {
            Some(s) => s,
            None => {
                warn!("impossible, unregistered session {}", sid);
                return;
            }
        };

        self.inner.dec_conn_count();
        if self.bootstraps.contains(&*session.peer.id) {
            // Increase bootstrap retry instead of removing it
            session.peer.mark_disconnected();
            session.peer.reset_retry();
            session.peer.set_retry(MAX_RETRY_COUNT);
        } else {
            self.inner.remove_peer(&session.peer.id);
        }

        self.disconnect_session(sid);
    }

    fn session_blocked(&self, ctx: Arc<SessionContext>) {
        warn!(
            "network: session {} blocked, pending data size {}",
            ctx.id,
            ctx.pending_data_size()
        );

        if let Some(session) = self.inner.session(ctx.id) {
            session.block();
        }
    }

    fn retry_peer_later(&self, pid: &PeerId) {
        info!("network: {:?}: retry peer later {:?}", self.peer_id, pid);

        let peer = match self.inner.peer(pid) {
            Some(p) => p,
            None => return,
        };

        if peer.connectedness() == Connectedness::Connected {
            let sid = peer.session_id();
            self.inner.remove_session(sid);
            self.inner.dec_conn_count();

            // Make sure we disconnect this peer
            self.disconnect_session(sid);
        }

        peer.mark_disconnected();
        peer.increase_retry();
    }

    fn connect_multiaddrs(&self, addrs: Vec<Multiaddr>) {
        info!("network: {:?}: connect addrs {:?}", self.peer_id, addrs);

        let connect_attempt = ConnectionEvent::Connect {
            addrs,
            proto: TargetProtocol::All,
        };

        if self.conn_tx.unbounded_send(connect_attempt).is_err() {
            error!("network: connection service exit");
        }
    }

    fn connect_peers_now(&self, peers: Vec<ArcPeer>) {
        let connectable = |p: &'_ ArcPeer| -> bool {
            let connectedness = p.connectedness();
            if connectedness != Connectedness::CanConnect {
                info!("network: peer {:?} connectedness {}", p.id, connectedness);
                return false;
            }
            true
        };

        let multiaddrs = peers.into_iter().filter(connectable).map(|p| {
            p.set_connectedness(Connectedness::Connecting);
            p.raw_multiaddrs()
        });
        let multiaddrs = multiaddrs.flatten().collect::<Vec<_>>();

        if multiaddrs.is_empty() {
            debug!("network: no peer is connectable");
        } else {
            self.connect_multiaddrs(multiaddrs);
        }
    }

    fn connect_peer_by_ids_now(&self, pids: Vec<PeerId>) {
        let mut peers = Vec::new();

        {
            let book = self.inner.peers.read();
            for pid in pids.iter() {
                if let Some(peer) = book.get(pid) {
                    peers.push(peer.clone());
                }
            }
        }

        self.connect_peers_now(peers);
    }

    fn discover_multiaddr(&mut self, addr: Multiaddr) {
        let peer_id = match peer_id_from_multiaddr!(addr) {
            Some(Ok(p)) => p,
            _ => return, // Ignore multiaddr without peer id included
        };

        let addr: AddrInfo = match addr.try_into() {
            Ok(a) => a,
            _ => return, // Already check peer id above
        };

        // Ignore our listen multiaddrs
        if self.inner.listen_contains(&addr) {
            return;
        }

        if let Some(peer) = self.inner.peer(&peer_id) {
            if !peer.contains_multiaddr(&addr) {
                // Verify this multiaddr by connecting to it, if result in
                // repeated conection, then we can add it to that peer
                self.unknown_addrs.insert(addr);
            }
        } else {
            self.unknown_addrs.insert(addr);
        }
    }

    fn dicover_multi_multiaddrs(&mut self, addrs: Vec<Multiaddr>) {
        for addr in addrs.into_iter() {
            self.discover_multiaddr(addr);
        }
    }

    fn identified_addrs(&self, pid: &PeerId, addrs: Vec<Multiaddr>) {
        info!(
            "network: {:?}: peer {:?} multi identified addrs {:?}",
            self.peer_id, pid, addrs
        );

        if let Some(peer) = self.inner.peer(pid) {
            // Make sure all addresses include peer id
            let addrs = addrs
                .into_iter()
                .map(|ma| PeerMultiaddr::new(ma, pid))
                .collect::<Vec<_>>();

            peer.add_multiaddrs(addrs);
        }
    }

    fn repeated_connection(&mut self, ty: ConnectionType, sid: SessionId, addr: Multiaddr) {
        info!(
            "network: {:?}: repeated session {:?}, ty {}, remote addr {:?}",
            self.peer_id, sid, ty, addr
        );

        let session = match self.inner.session(sid) {
            Some(s) => s,
            None => {
                error!("network: repeated connection but session {} not found", sid);
                return;
            }
        };

        let addr = PeerMultiaddr::new(addr, &session.peer.id);
        self.unknown_addrs.remove(&addr);

        // TODO: For ConnectionType::Inbound, records repeated count,
        // reduce that peer's score, eventually ban it for a while.
        if ty == ConnectionType::Inbound {
            return;
        }

        // Insert multiaddr
        session.peer.add_multiaddrs(vec![addr]);
    }

    fn unconnectable_multiaddr(&mut self, addr: Multiaddr) {
        self.unknown_addrs.remove(&addr);

        let peer_id = match peer_id_from_multiaddr!(addr) {
            Some(Ok(p)) => p,
            _ => {
                // All multiaddrs we dial have peer id included
                error!("network: unconnectable multiaddr without peer id");
                return;
            }
        };

        let addr = PeerMultiaddr::new(addr, &peer_id);
        if let Some(peer) = self.inner.peer(&peer_id) {
            // We keep bootstrap peer addresses
            if !self.bootstraps.contains(&peer_id) {
                peer.remove_multiaddr(&addr);
            }
            peer.increase_retry();
        }
    }

    fn reconnect_addr_later(&mut self, addr: Multiaddr) {
        if let Some(unknown) = self.unknown_addrs.take(&addr) {
            unknown.inc_retry();
            if !unknown.run_out_retry() {
                self.unknown_addrs.insert(unknown);
            }

            return;
        }

        let peer_id = match peer_id_from_multiaddr!(addr) {
            Some(Ok(p)) => p,
            _ => return,
        };

        if let Some(peer) = self.inner.peer(&peer_id) {
            let connectedness = peer.connectedness();
            if Connectedness::Connected == connectedness {
                // Peer already connected, doesn't need to reconnect address
                return;
            }

            peer.set_connectedness(Connectedness::CanConnect);
            peer.increase_retry();

            if peer.run_out_retry() {
                // We don't remove bootstrap peer or protected peer
                if self.bootstraps.contains(&*peer.id)
                    || self.inner.is_protected_by_chain_addr(&peer.chain_addr)
                {
                    peer.reset_retry();
                } else {
                    self.inner.remove_peer(&peer.id);
                }
            }
        }
    }

    fn process_event(&mut self, event: PeerManagerEvent) {
        match event {
            PeerManagerEvent::NewSession { pubkey, ctx, .. } => self.new_session(pubkey, ctx),
            PeerManagerEvent::SessionClosed { sid, .. } => self.session_closed(sid),
            PeerManagerEvent::PeerAlive { pid } => self.update_peer_alive(&pid),
            PeerManagerEvent::RemovePeerBySession { sid, .. } => self.remove_peer_by_session(sid),
            PeerManagerEvent::SessionBlocked { ctx, .. } => self.session_blocked(ctx),
            PeerManagerEvent::RetryPeerLater { pid, .. } => self.retry_peer_later(&pid),
            PeerManagerEvent::ConnectPeersNow { pids } => self.connect_peer_by_ids_now(pids),
            PeerManagerEvent::ProtectPeersByChainAddr { chain_addrs } => {
                self.inner.protect_peers_by_chain_addr(chain_addrs);
            }
            PeerManagerEvent::DiscoverAddr { addr } => self.discover_multiaddr(addr),
            PeerManagerEvent::DiscoverMultiAddrs { addrs } => self.dicover_multi_multiaddrs(addrs),
            PeerManagerEvent::IdentifiedAddrs { pid, addrs } => self.identified_addrs(&pid, addrs),
            // NOTE: Alice may disconnect to Bob, but bob didn't know
            // that, so the next time, Alice try to connect to Bob will
            // cause repeated connection. The only way to fix this right
            // now is wait for time out.
            PeerManagerEvent::RepeatedConnection { ty, sid, addr } => {
                self.repeated_connection(ty, sid, addr)
            }
            // TODO: ban unconnectable address for a while instead of repeated
            // connection attempts.
            PeerManagerEvent::UnconnectableAddress { addr, kind, .. } => {
                // Since io::Other is unexpected, it's ok warning here.
                warn!("unconnectable address {} {}", addr, kind);
                self.unconnectable_multiaddr(addr)
            }
            PeerManagerEvent::ReconnectAddrLater { addr, kind, .. } => {
                info!("reconnect address {} later {}", addr, kind);
                self.reconnect_addr_later(addr);
            }
            PeerManagerEvent::AddNewListenAddr { addr } => {
                let peer_addr = PeerMultiaddr::new(addr, &self.peer_id);
                self.unknown_addrs.remove(&peer_addr);
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

        let connected_addrs = self.connected_addrs();
        debug!(
            "network: {:?}: connected peer_addr(s) {}: {:?}",
            self.peer_id,
            connected_addrs.len(),
            connected_addrs
        );

        // Check connecting count
        let connection_count = self.inner.conn_count();
        if connection_count < self.config.max_connections {
            let remain_count = self.config.max_connections - connection_count;
            let unconnected_addrs = self.unconnected_addrs(remain_count);
            let candidate_count = unconnected_addrs.len();

            debug!(
                "network: {:?}: connections not fullfill, {} candidate addrs found",
                self.peer_id, candidate_count
            );

            if !unconnected_addrs.is_empty() {
                self.connect_multiaddrs(unconnected_addrs);
            }
        }

        // Clean expired whitelisted peer
        self.inner.whitelist.write().retain(|p| !p.is_expired());

        // Clean unknown addrs
        let run_out_retry = |addr: &AddrInfo| -> bool {
            if addr.is_timeout() {
                addr.inc_retry();
            }
            addr.run_out_retry()
        };
        self.unknown_addrs.retain(|ua| !run_out_retry(ua));

        Poll::Pending
    }
}
