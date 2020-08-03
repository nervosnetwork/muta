#![allow(clippy::mutable_key_type)]

mod addr_set;
mod peer;
mod retry;
mod save_restore;
mod session_book;
mod shared;
mod tags;
mod time;
mod trust_metric;

#[cfg(feature = "diagnostic")]
pub mod diagnostic;

use addr_set::PeerAddrSet;
use retry::Retry;
use save_restore::{NoPeerDatFile, PeerDatFile, SaveRestore};
use session_book::{AcceptableSession, ArcSession, SessionContext};
use tags::Tags;

pub use peer::{ArcPeer, Connectedness};
pub use session_book::SessionBook;
pub use shared::SharedSessions;
pub use trust_metric::{TrustMetric, TrustMetricConfig};

#[cfg(test)]
mod test_manager;

use std::borrow::Borrow;
use std::cmp::PartialEq;
use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::ops::Deref;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use derive_more::Display;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::stream::Stream;
use futures::task::AtomicWaker;
use log::{debug, error, info, warn};
use parking_lot::RwLock;
use protocol::traits::{PeerTag, TrustFeedback};
use rand::seq::IteratorRandom;
use serde_derive::{Deserialize, Serialize};
use tentacle::multiaddr::Multiaddr;
use tentacle::secio::{PeerId, PublicKey};
use tentacle::service::{SessionType, TargetProtocol};
use tentacle::SessionId;

use crate::common::{resolve_if_unspecified, HeartBeat};
use crate::error::{NetworkError, PeerIdNotFound};
use crate::event::{
    ConnectionErrorKind, ConnectionEvent, ConnectionType, MisbehaviorKind, PeerManagerEvent,
    SessionErrorKind,
};
use crate::traits::MultiaddrExt;

const SAME_IP_LIMIT_BAN: Duration = Duration::from_secs(5 * 60);
const REPEATED_CONNECTION_TIMEOUT: u64 = 30; // seconds
const BACKOFF_BASE: u64 = 2;
const MAX_RETRY_INTERVAL: u64 = 512; // seconds
const MAX_RETRY_COUNT: u8 = 30;
const SHORT_ALIVE_SESSION: u64 = 3; // seconds
const MAX_CONNECTING_MARGIN: usize = 10;

const GOOD_TRUST_SCORE: u8 = 80u8;
const WORSE_TRUST_SCALAR_RATIO: usize = 10;

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
        if Self::extract_id(&ma).is_some() {
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

struct Inner {
    our_id: Arc<PeerId>,

    sessions:  SessionBook,
    consensus: RwLock<HashSet<PeerId>>,
    peers:     RwLock<HashSet<ArcPeer>>,

    listen: RwLock<HashSet<PeerMultiaddr>>,
}

impl Inner {
    pub fn new(our_id: PeerId, sessions: SessionBook) -> Self {
        Inner {
            our_id: Arc::new(our_id),

            sessions,
            consensus: Default::default(),
            peers: Default::default(),

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
        self.sessions.len()
    }

    /// If peer exists, return false
    pub fn add_peer(&self, peer: ArcPeer) -> bool {
        self.peers.write().insert(peer)
    }

    pub fn peer_count(&self) -> usize {
        self.peers.read().len()
    }

    pub fn peer(&self, peer_id: &PeerId) -> Option<ArcPeer> {
        self.peers.read().get(peer_id).cloned()
    }

    pub fn contains(&self, peer_id: &PeerId) -> bool {
        self.peers.read().contains(peer_id)
    }

    pub fn connectable_peers<F>(&self, max: usize, addition_filter: F) -> Vec<ArcPeer>
    where
        F: Fn(&ArcPeer) -> bool + 'static,
    {
        let connectable = |p: &'_ &ArcPeer| -> bool {
            (p.connectedness() == Connectedness::NotConnected
                || p.connectedness() == Connectedness::CanConnect)
                && p.retry.ready()
                && p.multiaddrs.connectable_len() > 0
                && !p.banned()
                && addition_filter(p)
        };

        let mut rng = rand::thread_rng();
        let book = self.peers.read();
        let qualified_peers = book.iter().filter(connectable).map(ArcPeer::to_owned);

        qualified_peers.choose_multiple(&mut rng, max)
    }

    pub fn session(&self, sid: SessionId) -> Option<ArcSession> {
        self.sessions.get(&sid)
    }

    pub fn share_sessions(&self) -> Vec<ArcSession> {
        self.sessions.all()
    }

    pub fn remove_session(&self, sid: SessionId) -> Option<ArcSession> {
        self.sessions.remove(&sid)
    }

    pub fn package_peers(&self) -> Vec<ArcPeer> {
        self.peers.read().iter().cloned().collect()
    }

    fn restore(&self, peers: Vec<ArcPeer>) {
        self.peers.write().extend(peers);
    }
}

#[derive(Debug, Clone)]
pub struct PeerManagerConfig {
    /// Our Peer ID
    pub our_id: PeerId,

    /// Our public key
    pub pubkey: PublicKey,

    /// Bootstrap peers
    pub bootstraps: Vec<ArcPeer>,

    /// Always accept/connect peers in list
    pub allowlist:      Vec<PeerId>,
    /// Only accept/conect peers in allowlist
    pub allowlist_only: bool,

    /// Limit connections from same ip
    pub same_ip_conn_limit: usize,

    /// Trust metric config
    pub peer_trust_config: Arc<TrustMetricConfig>,
    pub peer_fatal_ban:    Duration,
    pub peer_soft_ban:     Duration,

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

    pub fn random_addrs(&self, max: usize, sid: SessionId) -> Vec<Multiaddr> {
        let mut rng = rand::thread_rng();
        let book = self.inner.peers.read();
        let peers = book.iter().choose_multiple(&mut rng, max);

        let is_self_consensus = self
            .inner
            .peer(&self.inner.our_id)
            .map(|p| p.tags.contains(&PeerTag::Consensus))
            .unwrap_or_else(|| false);

        let is_remote_consensus = self
            .inner
            .session(sid)
            .map(|s| s.peer.tags.contains(&PeerTag::Consensus))
            .unwrap_or_else(|| false);

        let condidates = peers
            .into_iter()
            .filter_map(|p| {
                if !is_remote_consensus && p.tags.contains(&PeerTag::Consensus) {
                    None
                } else {
                    Some(p.multiaddrs.all_raw())
                }
            })
            .flatten();

        if !is_self_consensus {
            // Should always include our self
            let our_self = self.listen_addrs();
            our_self.into_iter().chain(condidates).take(max).collect()
        } else {
            condidates.take(max).collect()
        }
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        let listen = self.inner.listen();
        debug_assert!(!listen.is_empty(), "listen should alway be set");

        let sanitize = |pma: PeerMultiaddr| -> Multiaddr {
            let ma: Multiaddr = pma.into();
            match resolve_if_unspecified(&ma) {
                Ok(resolved) => resolved,
                Err(_) => ma,
            }
        };

        listen.into_iter().map(sanitize).collect()
    }

    pub fn tag(&self, peer_id: &PeerId, tag: PeerTag) -> Result<(), NetworkError> {
        let consensus_tag = tag == PeerTag::Consensus;

        if let Some(peer) = self.inner.peer(peer_id) {
            peer.tags.insert(tag)?;
        } else {
            let peer = ArcPeer::new(peer_id.to_owned());
            peer.tags.insert(tag)?;
            self.inner.add_peer(peer);
        }

        if consensus_tag {
            self.inner.consensus.write().insert(peer_id.to_owned());
        }

        Ok(())
    }

    pub fn untag(&self, peer_id: &PeerId, tag: &PeerTag) {
        if let Some(peer) = self.inner.peer(peer_id) {
            peer.tags.remove(tag);
        }

        if tag == &PeerTag::Consensus {
            self.inner.consensus.write().remove(peer_id);
        }
    }

    pub fn tag_consensus(&self, peer_ids: Vec<PeerId>) {
        {
            for peer_id in self.inner.consensus.read().iter() {
                if let Some(peer) = self.inner.peer(peer_id) {
                    peer.tags.remove(&PeerTag::Consensus)
                }
            }
        }

        for peer_id in peer_ids.iter() {
            let _ = self.tag(peer_id, PeerTag::Consensus);
        }

        {
            let id_set = HashSet::from_iter(peer_ids);
            *self.inner.consensus.write() = id_set;
        }
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

    // diagnostic event hook
    #[cfg(feature = "diagnostic")]
    diagnostic_hook: Option<diagnostic::DiagnosticHookFn>,
}

impl PeerManager {
    pub fn new(
        config: PeerManagerConfig,
        event_rx: UnboundedReceiver<PeerManagerEvent>,
        conn_tx: UnboundedSender<ConnectionEvent>,
    ) -> Self {
        let peer_id = config.our_id.clone();
        let session_config = session_book::Config::from(&config);
        let session_book = SessionBook::new(session_config);

        let inner = Arc::new(Inner::new(config.our_id.clone(), session_book));
        let bootstraps = HashSet::from_iter(config.bootstraps.clone());
        let waker = Arc::new(AtomicWaker::new());
        let heart_beat = HeartBeat::new(Arc::clone(&waker), config.routine_interval);
        let peer_dat_file = Box::new(NoPeerDatFile);

        for peer_id in config.allowlist.iter().cloned() {
            assert_eq!(inner.peer_count(), 0, "should be empty before bootstrapped");

            let peer = ArcPeer::new(peer_id);
            let _ = peer.tags.insert(PeerTag::AlwaysAllow);

            inner.add_peer(peer);
        }

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

            #[cfg(feature = "diagnostic")]
            diagnostic_hook: None,
        }
    }

    pub fn handle(&self) -> PeerManagerHandle {
        PeerManagerHandle {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn share_session_book(&self, config: shared::Config) -> SharedSessions {
        SharedSessions::new(Arc::clone(&self.inner), config)
    }

    #[cfg(feature = "diagnostic")]
    pub fn register_diagnostic_hook(&mut self, f: diagnostic::DiagnosticHookFn) {
        self.diagnostic_hook = Some(f);
    }

    #[cfg(feature = "diagnostic")]
    pub fn diagnostic(&self) -> diagnostic::Diagnostic {
        diagnostic::Diagnostic::new(Arc::clone(&self.inner))
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

    #[cfg(test)]
    fn inner(&self) -> Arc<Inner> {
        Arc::clone(&self.inner)
    }

    #[cfg(test)]
    fn config(&self) -> PeerManagerConfig {
        self.config.clone()
    }

    #[cfg(test)]
    fn set_connecting(&mut self, peers: Vec<ArcPeer>) {
        for peer in peers.into_iter() {
            self.connecting.insert(ConnectingAttempt::new(peer));
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
            if let Err(e) = remote_peer.set_pubkey(pubkey) {
                error!("impossible, set public key failed {}", e);
                error!("new session without peer pubkey, chain book will not be updated");
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

        if remote_peer.banned() {
            info!("banned peer {:?} incomming", remote_peer_id);
            remote_peer.mark_disconnected();
            self.disconnect_session(ctx.id);
            return;
        }

        if self.config.allowlist_only
            && !remote_peer.tags.contains(&PeerTag::AlwaysAllow)
            && !remote_peer.tags.contains(&PeerTag::Consensus)
        {
            debug!("allowlist_only enabled, reject peer {:?}", remote_peer.id);
            remote_peer.mark_disconnected();
            self.disconnect_session(ctx.id);
            return;
        }

        if self.inner.connected() >= self.config.max_connections {
            let found_replacement = || -> bool {
                let incoming_trust_score = match remote_peer.trust_metric() {
                    Some(trust_metric) => trust_metric.trust_score(),
                    None => return false,
                };

                for session in self.inner.share_sessions() {
                    let session_trust_score = match session.peer.trust_metric() {
                        Some(trust_metric) => trust_metric.trust_score(),
                        None => {
                            // Impossible
                            error!("session peer {:?} trust metric not found", session.peer.id);
                            return false;
                        }
                    };

                    // Ensure that session be replaced has traveled enough
                    // intervals
                    if incoming_trust_score > session_trust_score
                        && !session.peer.tags.contains(&PeerTag::AlwaysAllow)
                        && !session.peer.tags.contains(&PeerTag::Consensus)
                        && session.peer.alive()
                            > self.config.peer_trust_config.interval().as_secs() * 20
                    {
                        self.disconnect_session(session.id);
                        return true;
                    }
                }

                false
            };

            if !remote_peer.tags.contains(&PeerTag::AlwaysAllow)
                && !remote_peer.tags.contains(&PeerTag::Consensus)
                && !found_replacement()
            {
                remote_peer.mark_disconnected();
                self.disconnect_session(ctx.id);
                return;
            }
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

        // Currently we only save accepted peer.
        // NOTE: We have to save peer first to be able to ban it. Check out
        // SAME_IP_LIMIT_BAN.
        // TODO: save to database
        if !self.inner.contains(&remote_peer_id) {
            self.inner.add_peer(remote_peer.clone());
        }

        // Always allow peer in allowlist and consensus peer
        if !remote_peer.tags.contains(&PeerTag::AlwaysAllow)
            && !remote_peer.tags.contains(&PeerTag::Consensus)
        {
            if let Err(err) = self.inner.sessions.acceptable(&session) {
                warn!("session {} unacceptable {}", ctx.id, err);

                // Ban this peer for a while so we won't choose it again
                // NOTE: Always allowed and consensus peer cannot be banned.
                if let Err(err) = remote_peer.tags.insert_ban(SAME_IP_LIMIT_BAN) {
                    warn!("ban same ip peer {:?} failed: {}", remote_peer.id, err);
                }

                remote_peer.mark_disconnected();
                self.disconnect_session(ctx.id);
                return;
            }
        }

        self.inner.sessions.insert(AcceptableSession(session));
        remote_peer.mark_connected(ctx.id);

        match remote_peer.trust_metric() {
            Some(trust_metric) => trust_metric.start(),
            None => {
                let trust_metric = TrustMetric::new(Arc::clone(&self.config.peer_trust_config));
                trust_metric.start();

                remote_peer.set_trust_metric(trust_metric);
            }
        }
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

        match session.peer.trust_metric() {
            Some(trust_metric) => trust_metric.pause(),
            None => {
                warn!("session peer {:?} trust metric not found", session.peer.id);

                let trust_metric = TrustMetric::new(Arc::clone(&self.config.peer_trust_config));
                session.peer.set_trust_metric(trust_metric);
            }
        }

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

    fn connect_failed(&mut self, addr: Multiaddr, error_kind: ConnectionErrorKind) {
        use ConnectionErrorKind::{
            DNSResolver, Io, MultiaddrNotSuppored, PeerIdNotMatch, ProtocolHandle, SecioHandshake,
        };

        let peer_addr: PeerMultiaddr = match addr.clone().try_into() {
            Ok(pma) => pma,
            Err(e) => {
                // All multiaddrs we dial have peer id included
                error!("unconnectable multiaddr {} without peer id {}", addr, e);
                return;
            }
        };

        let peer_id = peer_addr.peer_id();
        let peer = match self.inner.peer(&peer_id) {
            Some(p) => p,
            None => {
                // Impossibe
                error!("outbound connecting peer not found {:?}", peer_id);
                return;
            }
        };

        match error_kind {
            Io(_) | DNSResolver(_) => peer.multiaddrs.inc_failure(&peer_addr),
            MultiaddrNotSuppored(_) => {
                info!("give up unsupported multiaddr {}", addr);
                peer.multiaddrs.give_up(&peer_addr);
            }
            PeerIdNotMatch => {
                warn!("give up multiaddr {} because peer id not match", peer_addr);
                peer.multiaddrs.give_up(&peer_addr);
            }
            SecioHandshake(_) | ProtocolHandle => {
                warn!("give up peer {:?} becasue {}", peer.id, error_kind);
                peer.set_connectedness(Connectedness::Unconnectable);
            }
        }

        if let Some(attempt) = self.connecting.take(&peer_id) {
            if attempt.peer.connectedness() == Connectedness::Unconnectable {
                // We already give up peer
                return;
            }

            attempt.complete_one_multiaddr();
            // No more connecting multiaddrs from this peer
            // This means all multiaddrs failure
            if attempt.multiaddrs() == 0 {
                attempt.peer.retry.inc();
                attempt.peer.set_connectedness(Connectedness::CanConnect);

                if attempt.peer.retry.run_out() {
                    attempt.peer.set_connectedness(Connectedness::Unconnectable);
                }

            // FIXME
            // if let Some(trust_metric) = attempt.peer.trust_metric() {
            //     trust_metric.bad_events(1);
            // }
            } else {
                // Wait for other connecting multiaddrs result
                self.connecting.insert(attempt);
            }
        }
    }

    fn session_failed(&self, sid: SessionId, error_kind: SessionErrorKind) {
        use SessionErrorKind::{Io, Protocol, Unexpected};

        debug!("session {} failed", sid);

        let session = match self.inner.remove_session(sid) {
            Some(s) => s,
            None => return, /* Session may be removed by other event or rejected
                             * due to max connections before insert */
        };
        // Ensure we disconnect this peer
        self.disconnect_session(sid);
        session.peer.mark_disconnected();

        match session.peer.trust_metric() {
            Some(trust_metric) => trust_metric.bad_events(1),
            None => {
                warn!("session peer {:?} trust metric not found", session.peer.id);

                let trust_metric = TrustMetric::new(Arc::clone(&self.config.peer_trust_config));
                trust_metric.bad_events(1);

                session.peer.set_trust_metric(trust_metric);
            }
        }

        match error_kind {
            Io(_) => session.peer.retry.inc(),
            Protocol { .. } | Unexpected(_) => {
                let pid = &session.peer.id;
                let remote_addr = &session.connected_addr;

                warn!("give up peer {:?} from {} {}", pid, remote_addr, error_kind);
                session.peer.set_connectedness(Connectedness::Unconnectable);
            }
        }
    }

    fn update_peer_alive(&self, pid: &PeerId) {
        if let Some(peer) = self.inner.peer(pid) {
            let sid = peer.session_id();
            if sid != 0.into() {
                if let Some(session) = self.inner.session(sid) {
                    info!("peer {:?} {} alive", pid, session.connected_addr);
                }
            }

            peer.retry.reset(); // Just in case
            peer.update_alive();
        }
    }

    fn peer_misbehave(&self, pid: PeerId, kind: MisbehaviorKind) {
        use MisbehaviorKind::{Discovery, PingTimeout, PingUnexpect};

        let peer = match self.inner.peer(&pid) {
            Some(p) => p,
            None => {
                error!("misbehave peer {:?} not found", pid);
                return;
            }
        };

        match peer.trust_metric() {
            Some(trust_metric) => trust_metric.bad_events(1),
            None => {
                warn!("session peer {:?} trust metric not found", peer.id);

                let trust_metric = TrustMetric::new(Arc::clone(&self.config.peer_trust_config));
                trust_metric.start();
                trust_metric.bad_events(1);

                peer.set_trust_metric(trust_metric);
            }
        }

        let sid = peer.session_id();
        if sid == SessionId::new(0) {
            // Impossible, connected session always bigger than 0
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

    fn trust_metric_feedback(&self, pid: PeerId, feedback: TrustFeedback) {
        use TrustFeedback::{Bad, Fatal, Good, Neutral, Worse};

        let peer = match self.inner.peer(&pid) {
            Some(p) => p,
            None => {
                error!("fatal peer {:?} not found", pid);
                return;
            }
        };

        let peer_trust_metric = match peer.trust_metric() {
            Some(t) => t,
            None => {
                warn!("session peer {:?} trust metric not found", peer.id);

                let trust_metric = TrustMetric::new(Arc::clone(&self.config.peer_trust_config));
                trust_metric.start();

                peer.set_trust_metric(trust_metric.clone());
                trust_metric
            }
        };

        match &feedback {
            Fatal(reason) => {
                warn!("peer {:?} trust feedback fatal {}", pid, reason);
                if peer.tags.contains(&PeerTag::AlwaysAllow)
                    || peer.tags.contains(&PeerTag::Consensus)
                {
                    return;
                }

                let fatal_ban = self.config.peer_fatal_ban;
                info!("peer {:?} ban {} seconds", pid, fatal_ban.as_secs());
                peer_trust_metric.pause();
                if let Err(e) = peer.tags.insert_ban(fatal_ban) {
                    warn!("ban peer {}", e);
                    debug!("impossible, we already make sure peer isn't in allowlist");
                }

                if let Some(session) = self.inner.remove_session(peer.session_id()) {
                    self.disconnect_session(session.id);
                }
                peer.mark_disconnected();
            }
            Bad(_) | Worse(_) => {
                match &feedback {
                    Bad(reason) => {
                        info!("peer {:?} trust feedback bad {}", pid, reason);
                        peer_trust_metric.bad_events(1);
                    }
                    Worse(reason) => {
                        warn!("peer {:?} trust feedback worse {}", pid, reason);
                        peer_trust_metric.bad_events(WORSE_TRUST_SCALAR_RATIO);
                    }
                    _ => unreachable!(),
                };

                if peer_trust_metric.knock_out()
                    && !peer.tags.contains(&PeerTag::AlwaysAllow)
                    && !peer.tags.contains(&PeerTag::Consensus)
                {
                    let soft_ban = self.config.peer_soft_ban.as_secs();
                    info!("peer {:?} knocked out, soft ban {} seconds", pid, soft_ban);

                    peer_trust_metric.pause();
                    if let Err(e) = peer.tags.insert_ban(Duration::from_secs(soft_ban)) {
                        warn!("ban peer {}", e);
                        debug!("impossible, we already make sure peer isn't in allowlist");
                    }

                    if let Some(session) = self.inner.remove_session(peer.session_id()) {
                        self.disconnect_session(session.id);
                    }
                    peer.mark_disconnected();
                }
            }
            Neutral => (),
            Good => peer_trust_metric.good_events(1),
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

            match session.peer.trust_metric() {
                Some(trust_metric) => trust_metric.bad_events(1),
                None => {
                    warn!("session peer {:?} trust metric not found", session.peer.id);

                    let trust_metric = TrustMetric::new(Arc::clone(&self.config.peer_trust_config));
                    trust_metric.start();
                    trust_metric.bad_events(1);

                    session.peer.set_trust_metric(trust_metric);
                }
            };
        }
    }

    fn connect_peers_now(&mut self, peers: Vec<ArcPeer>) {
        let peer_addrs = peers.into_iter().map(|peer| {
            peer.set_connectedness(Connectedness::Connecting);

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
            if self.config.allowlist_only
                && !p.tags.contains(&PeerTag::AlwaysAllow)
                && !p.tags.contains(&PeerTag::Consensus)
            {
                debug!("filter peer {:?} not in allowlist", p.id);
                return None;
            }

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

        let connectable_peers: Vec<_> = peers.into_iter().filter_map(connectable).collect();

        if !connectable_peers.is_empty() {
            self.connect_peers_now(connectable_peers);
        }
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

            self.inner.add_peer(new_peer);
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
        match ty {
            ConnectionType::Inbound => session.peer.multiaddrs.remove(&peer_addr),
            ConnectionType::Outbound => session.peer.multiaddrs.reset_failure(&peer_addr),
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
            PeerManagerEvent::TrustMetric { pid, feedback } => {
                self.trust_metric_feedback(pid, feedback)
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
            futures::pin_mut!(event_rx);

            // service ready in common
            let event = crate::service_ready!("peer manager", event_rx.poll_next(ctx));
            log::debug!("network: {:?}: event {}", self.peer_id, event);

            #[cfg(feature = "diagnostic")]
            let diag_event: Option<diagnostic::DiagnosticEvent> = From::from(&event);

            self.process_event(event);

            #[cfg(feature = "diagnostic")]
            if let (Some(hook), Some(event)) = (self.diagnostic_hook.as_ref(), diag_event) {
                hook(event)
            }
        }

        // Check connecting count
        let connected_count = self.inner.connected();
        let connection_attempts = connected_count + self.connecting.len();
        let max_connection_attempts = self.config.max_connections + MAX_CONNECTING_MARGIN;

        if connected_count < self.config.max_connections
            && connection_attempts < max_connection_attempts
        {
            let filter_good_peer = |peer: &ArcPeer| -> bool {
                if let Some(trust_metric) = peer.trust_metric() {
                    trust_metric.trust_score() > GOOD_TRUST_SCORE
                } else {
                    false
                }
            };
            let just_enough = |_: &ArcPeer| -> bool { true };

            let remain_count = max_connection_attempts - connection_attempts;
            let mut connectable_peers =
                self.inner.connectable_peers(remain_count, filter_good_peer);
            if connectable_peers.is_empty() {
                connectable_peers = self.inner.connectable_peers(remain_count, just_enough);
            }
            let candidate_count = connectable_peers.len();

            debug!(
                "network: {:?}: connections not fullfill, {} candidate peers found",
                self.peer_id, candidate_count
            );

            if !connectable_peers.is_empty() {
                self.connect_peers(connectable_peers);
            }
        }

        Poll::Pending
    }
}
