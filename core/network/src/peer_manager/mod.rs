mod book;
mod disc;
mod ident;
mod peer;
mod save_restore;

use save_restore::{NoPeerDatFile, PeerDatFile, SaveRestore};

pub use book::{SharedSessions, SharedSessionsConfig};
pub use disc::DiscoveryAddrManager;
pub use ident::IdentifyCallback;
pub use peer::{ArcPeer, Connectedness};

use std::{
    borrow::Borrow,
    cmp::PartialEq,
    collections::HashSet,
    future::Future,
    hash::{Hash, Hasher},
    iter::FromIterator,
    ops::Deref,
    path::PathBuf,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

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
use tentacle::{
    context::SessionContext,
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
    service::{SessionType, TargetProtocol},
    SessionId,
};

use crate::{
    common::{ConnectedAddr, HeartBeat},
    error::NetworkError,
    event::{ConnectionEvent, ConnectionType, PeerManagerEvent},
    traits::{MultiaddrExt, PeerQuerier},
};

macro_rules! peer_id_from_multiaddr {
    ($multiaddr:expr) => {
        $multiaddr
            .peer_id_bytes()
            .map(|bs| PeerId::from_bytes(bs.to_vec()))
    };
}

const MAX_RETRY_COUNT: u8 = 30;
const ALIVE_RETRY_INTERVAL: u64 = 3; // seconds

#[derive(Debug, Clone)]
pub struct UnknownAddr {
    addr:        Multiaddr,
    connecting:  Arc<AtomicBool>,
    retry_count: u8,
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

impl Borrow<Multiaddr> for UnknownAddr {
    fn borrow(&self) -> &Multiaddr {
        &self.addr
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

    sessions: RwLock<HashSet<ArcSession>>,
    peers:    RwLock<HashSet<ArcPeer>>,
    chain:    RwLock<HashSet<ArcPeerByChain>>,

    id:     Arc<PeerId>,
    listen: RwLock<Option<Multiaddr>>,
}

impl Inner {
    pub fn new(id: PeerId) -> Self {
        Inner {
            connection_count: AtomicUsize::new(0),

            sessions: Default::default(),
            peers:    Default::default(),
            chain:    Default::default(),

            id:     Arc::new(id),
            listen: Default::default(),
        }
    }

    pub fn set_listen(&self, mut multiaddr: Multiaddr) {
        if !multiaddr.has_peer_id() {
            multiaddr.push_id(self.id.as_ref().to_owned());
        }

        *self.listen.write() = Some(multiaddr);
    }

    pub fn listen(&self) -> Option<Multiaddr> {
        self.listen.read().clone()
    }

    pub fn inc_conn_count(&self) {
        self.connection_count.fetch_add(1, Ordering::SeqCst);
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

    pub fn session(&self, sid: &SessionId) -> Option<ArcSession> {
        self.sessions.read().get(sid).cloned()
    }

    pub fn share_sessions(&self) -> Vec<ArcSession> {
        self.sessions.read().iter().cloned().collect()
    }

    pub fn remove_session(&self, sid: &SessionId) -> Option<ArcSession> {
        self.sessions.write().take(sid)
    }

    pub fn register_self(&self, pubkey: PublicKey) -> Result<(), NetworkError> {
        let peer = ArcPeer::from_pubkey(pubkey)?;
        peer.set_connectedness(Connectedness::Connected);

        Ok(self.add_peer(peer))
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

        peers
            .into_iter()
            .map(|p| p.multiaddrs())
            .flatten()
            .collect()
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        let listen = self.inner.listen();
        debug_assert!(listen.is_some(), "listen should alway be set");

        listen.map(|addr| vec![addr]).unwrap_or_else(Vec::new)
    }
}

impl PeerQuerier for PeerManagerHandle {
    fn connected_addr(&self, pid: &PeerId) -> Option<ConnectedAddr> {
        if let Some(peer) = self.inner.peer(pid) {
            if let Some(session) = self.inner.session(&peer.session_id()) {
                return Some(session.connected_addr.to_owned());
            }
        }

        None
    }

    fn connected_peers(&self) -> Vec<PeerId> {
        self.inner
            .share_sessions()
            .into_iter()
            .map(|s| s.peer.id.as_ref().to_owned())
            .collect()
    }

    fn pending_data_size(&self, pid: &PeerId) -> usize {
        if let Some(peer) = self.inner.peer(pid) {
            if let Some(session) = self.inner.session(&peer.session_id()) {
                return session.ctx.pending_data_size();
            }
        }

        0
    }
}

pub struct PeerManager {
    // core peer pool
    inner:      Arc<Inner>,
    config:     PeerManagerConfig,
    peer_id:    PeerId,
    bootstraps: HashSet<ArcPeer>,

    // unknown
    unknown_addrs: HashSet<UnknownAddr>,

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

        let inner = Arc::new(Inner::new(peer_id.clone()));
        let bootstraps = HashSet::from_iter(config.bootstraps.clone());
        let waker = Arc::new(AtomicWaker::new());
        let heart_beat = HeartBeat::new(Arc::clone(&waker), config.routine_interval);
        let peer_dat_file = Box::new(NoPeerDatFile);

        // Register our self
        inner
            .register_self(config.pubkey.clone())
            .expect("register self public key");

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

    pub fn share_session_book(&self, config: SharedSessionsConfig) -> SharedSessions {
        SharedSessions::new(Arc::clone(&self.inner), config)
    }

    pub fn enable_save_restore(&mut self) {
        let peer_dat_file = PeerDatFile::new(&self.config.peer_dat_file);

        self.peer_dat_file = Box::new(peer_dat_file);
    }

    pub fn restore_peers(&self) -> Result<(), NetworkError> {
        let peers = self.peer_dat_file.restore()?;
        Ok(self.inner.restore(peers))
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
        }

        self.connect_peers(peers.clone());
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
            let addrs = condidate_peers.iter().map(|p| {
                p.set_connectedness(Connectedness::Connecting);
                p.multiaddrs()
            });

            condidates = addrs.flatten().collect();
            remain -= condidate_peers.len()
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

    fn unconnected_unknowns(&self, max: usize) -> Vec<&UnknownAddr> {
        self.unknown_addrs
            .iter()
            .filter(|unknown| !unknown.connecting())
            .take(max)
            .collect()
    }

    fn new_session(&mut self, pubkey: PublicKey, ctx: Arc<SessionContext>) {
        self.unknown_addrs.remove(&ctx.address);

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

        let connectedness = peer.connectedness();
        if connectedness != Connectedness::Connecting || connectedness != Connectedness::Connected {
            self.inner.inc_conn_count();
        }

        let session = ArcSession::new(peer.clone(), Arc::clone(&ctx));
        self.inner.sessions.write().insert(session);
        peer.mark_connected(ctx.id);

        // Update peer multiaddr
        match ctx.ty {
            SessionType::Inbound => {
                // We can't use inbound address to connect to this peer
                debug!("network: {:?}: inbound {:?}", self.peer_id, ctx.address);
            }
            SessionType::Outbound => {
                let mut multiaddr = ctx.address.to_owned();
                if !multiaddr.has_peer_id() {
                    multiaddr.push_id(remote_peer_id);
                }

                self.unknown_addrs.remove(&multiaddr);
                peer.add_multiaddrs(vec![multiaddr]);
            }
        }
    }

    fn session_closed(&mut self, sid: SessionId) {
        info!("network: session {} closed", sid);

        let session = match self.inner.remove_session(&sid) {
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
        let session = match self.inner.remove_session(&sid) {
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

        let disconnect_peer = ConnectionEvent::Disconnect(sid);
        if self.conn_tx.unbounded_send(disconnect_peer).is_err() {
            debug!("network: connection service exit");
        }
    }

    fn session_blocked(&self, ctx: Arc<SessionContext>) {
        warn!(
            "network: session {} blocked, pending data size {}",
            ctx.id,
            ctx.pending_data_size()
        );

        if let Some(session) = self.inner.session(&ctx.id) {
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
            self.inner.remove_session(&sid);
            self.inner.dec_conn_count();

            // Make sure we disconnect this peer
            let disconnect_peer = ConnectionEvent::Disconnect(sid);
            if self.conn_tx.unbounded_send(disconnect_peer).is_err() {
                debug!("network: connection service exit");
            }
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
            error!("network: connection service offline");
        }
    }

    fn connect_peers(&self, peers: Vec<ArcPeer>) {
        let multiaddrs = peers
            .into_iter()
            .filter(|p| {
                let connectedness = p.connectedness();
                if connectedness != Connectedness::CanConnect {
                    info!("network: peer {:?} connectedness {}", p.id, connectedness);
                    return false;
                }
                if !p.retry_ready() {
                    let eta = p.next_attempt_since_now();
                    info!("network: peer {:?} isn't ready, ETA {} seconds", p.id, eta);
                    return false;
                }
                true
            })
            .map(|p| {
                p.set_connectedness(Connectedness::Connecting);
                p.multiaddrs()
            });

        self.connect_multiaddrs(multiaddrs.flatten().collect());
    }

    fn connect_peer_ids(&self, pids: Vec<PeerId>) {
        let mut peers = Vec::new();

        {
            let book = self.inner.peers.read();
            for pid in pids.iter() {
                if let Some(peer) = book.get(pid) {
                    peers.push(peer.clone());
                }
            }
        }

        self.connect_peers(peers);
    }

    fn discover_multiaddr(&mut self, addr: Multiaddr) {
        let peer_id = match peer_id_from_multiaddr!(addr) {
            Some(Ok(p)) => p,
            _ => return, // Ignore multiaddr without peer id included
        };

        if let Some(peer) = self.inner.peer(&peer_id) {
            if !peer.contains_multiaddr(&addr) {
                // Verify this multiaddr by connecting to it, if result in
                // repeated conection, then we can add it to that peer
                self.unknown_addrs.insert(addr.into());
            }
        } else {
            self.unknown_addrs.insert(addr.into());
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
                .map(|mut ma| {
                    if !ma.has_peer_id() {
                        ma.push_id(pid.to_owned())
                    }
                    ma
                })
                .collect::<Vec<_>>();

            peer.add_multiaddrs(addrs);
        }
    }

    fn repeated_connection(&mut self, ty: ConnectionType, sid: SessionId, mut addr: Multiaddr) {
        info!(
            "network: {:?}: repeated session {:?}, ty {}, remote addr {:?}",
            self.peer_id, sid, ty, addr
        );

        self.unknown_addrs.remove(&addr);

        // TODO: For ConnectionType::Listen, records repeated count,
        // reduce that peer's score, eventually ban it for a while.
        if ty == ConnectionType::Listen {
            return;
        }

        if let Some(session) = self.inner.session(&sid) {
            if !addr.has_peer_id() {
                addr.push_id(session.peer.id.as_ref().to_owned());
            }

            self.unknown_addrs.remove(&addr);
            session.peer.add_multiaddrs(vec![addr]);
        }
    }

    fn unconnectable_multiaddr(&mut self, addr: Multiaddr) {
        self.unknown_addrs.remove(&addr);

        let peer_id = match peer_id_from_multiaddr!(addr) {
            Some(Ok(p)) => p,
            _ => return,
        };

        if let Some(peer) = self.inner.peer(&peer_id) {
            // For bootstrap peer, we'll keep at least one multiaddr.
            if !self.bootstraps.contains(&peer_id) || peer.multiaddrs_len() > 1 {
                peer.remove_multiaddr(&addr);
            }
            peer.increase_retry();
        }
    }

    fn reconnect_addr_later(&mut self, addr: Multiaddr) {
        if let Some(mut unknown) = self.unknown_addrs.take(&addr) {
            unknown.set_connecting(false);
            unknown.increase_retry_count();

            if !unknown.reach_max_retry() {
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

            // TODO: Remove address that retry many times
            peer.set_connectedness(Connectedness::CanConnect);
            peer.increase_retry();
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
            PeerManagerEvent::ConnectPeers { pids } => self.connect_peer_ids(pids),
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
            PeerManagerEvent::AddListenAddr { mut addr } => {
                if !addr.has_peer_id() {
                    addr.push_id(self.peer_id.to_owned());
                }

                // TODO: listen on multi ports?
                if let Some(peer) = self.inner.peer(&self.peer_id) {
                    peer.set_multiaddrs(vec![addr.clone()]);
                }
                self.inner.set_listen(addr);
            }
            PeerManagerEvent::RemoveListenAddr { mut addr } => {
                if !addr.has_peer_id() {
                    addr.push_id(self.peer_id.to_owned());
                }

                if let Some(peer) = self.inner.peer(&self.peer_id) {
                    peer.remove_multiaddr(&addr);
                }
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

        Poll::Pending
    }
}
