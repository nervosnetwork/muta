use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::{
        atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use derive_more::Display;
use parking_lot::RwLock;
use protocol::{types::Address, Bytes};
use tentacle::{
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
    SessionId,
};

use crate::{error::ErrorKind, traits::MultiaddrExt};

pub const BACKOFF_BASE: u64 = 2;
pub const MAX_RETRY_INTERVAL: u64 = 512; // seconds
pub const VALID_ATTEMPT_INTERVAL: u64 = 4;

const CONNECTEDNESS_MASK: usize = 0b110;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Display)]
#[repr(usize)]
pub enum Connectedness {
    #[display(fmt = "can connect")]
    CanConnect = 0 << 1,

    #[display(fmt = "connecting")]
    Connecting = 1 << 1,

    #[display(fmt = "connected")]
    Connected = 2 << 1,

    #[display(fmt = "unconnectable")]
    Unconnectable = 3 << 1,
}

impl From<usize> for Connectedness {
    fn from(src: usize) -> Connectedness {
        use self::Connectedness::*;

        debug_assert!(
            src == CanConnect as usize
                || src == Connecting as usize
                || src == Connected as usize
                || src == Unconnectable as usize
        );

        unsafe { ::std::mem::transmute(src) }
    }
}

impl From<Connectedness> for usize {
    fn from(src: Connectedness) -> usize {
        let v = src as usize;
        debug_assert!(v & CONNECTEDNESS_MASK == v);
        v
    }
}

#[derive(Debug)]
pub struct Peer {
    pub id:          Arc<PeerId>,
    pub pubkey:      Arc<PublicKey>,
    multiaddrs:      RwLock<HashSet<Multiaddr>>,
    pub chain_addr:  Arc<Address>,
    connectedness:   AtomicUsize,
    session_id:      AtomicUsize,
    retry:           AtomicU8,
    next_attempt:    AtomicU64,
    connected_at:    AtomicU64,
    disconnected_at: AtomicU64,
    attempt_at:      AtomicU64,
    alive:           AtomicU64,
}

impl Peer {
    pub fn from_pubkey(pubkey: PublicKey) -> Result<Self, ErrorKind> {
        let chain_addr = Peer::pubkey_to_chain_addr(&pubkey)?;

        let peer = Peer {
            id:              Arc::new(pubkey.peer_id()),
            pubkey:          Arc::new(pubkey),
            multiaddrs:      RwLock::new(HashSet::new()),
            chain_addr:      Arc::new(chain_addr),
            connectedness:   AtomicUsize::new(Connectedness::CanConnect as usize),
            session_id:      AtomicUsize::new(0),
            retry:           AtomicU8::new(0),
            next_attempt:    AtomicU64::new(0),
            connected_at:    AtomicU64::new(0),
            disconnected_at: AtomicU64::new(0),
            attempt_at:      AtomicU64::new(0),
            alive:           AtomicU64::new(0),
        };

        Ok(peer)
    }

    fn validate_multiaddr(pid: &PeerId, ma: &Multiaddr) -> bool {
        ma.has_peer_id() && ma.peer_id_bytes() == Some(Cow::Borrowed(pid.as_bytes()))
    }

    /// # note: we only accept multiaddr with peer id included
    pub fn set_multiaddrs(&self, multiaddrs: Vec<Multiaddr>) {
        let multiaddrs = multiaddrs
            .into_iter()
            .filter(|ma| Self::validate_multiaddr(self.id.as_ref(), ma))
            .collect::<HashSet<_>>();

        *self.multiaddrs.write() = multiaddrs;
    }

    /// # note: we only accept multiaddr with peer id included
    pub fn add_multiaddrs(&self, multiaddrs: Vec<Multiaddr>) {
        let multiaddrs = multiaddrs
            .into_iter()
            .filter(|ma| Self::validate_multiaddr(self.id.as_ref(), ma))
            .collect::<Vec<_>>();

        self.multiaddrs.write().extend(multiaddrs)
    }

    pub fn remove_multiaddr(&self, multiaddr: &Multiaddr) {
        self.multiaddrs.write().remove(multiaddr);
    }

    pub fn contains_multiaddr(&self, multiaddr: &Multiaddr) -> bool {
        self.multiaddrs.read().contains(multiaddr)
    }

    pub fn multiaddrs(&self) -> Vec<Multiaddr> {
        self.multiaddrs.read().iter().cloned().collect()
    }

    pub fn multiaddrs_len(&self) -> usize {
        self.multiaddrs.read().len()
    }

    pub fn connectedness(&self) -> Connectedness {
        Connectedness::from(self.connectedness.load(Ordering::SeqCst))
    }

    pub fn set_connectedness(&self, flag: Connectedness) {
        self.connectedness
            .store(usize::from(flag), Ordering::SeqCst);
    }

    pub fn set_session_id(&self, sid: SessionId) {
        self.session_id.store(sid.value(), Ordering::SeqCst);
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id.load(Ordering::SeqCst).into()
    }

    pub fn next_attempt(&self) -> u64 {
        self.next_attempt.load(Ordering::SeqCst)
    }

    pub fn next_attempt_since_now(&self) -> u64 {
        let next_attempt =
            UNIX_EPOCH + Duration::from_secs(self.next_attempt.load(Ordering::SeqCst));

        duration_since(next_attempt, SystemTime::now()).as_secs()
    }

    pub fn connected_at(&self) -> u64 {
        self.connected_at.load(Ordering::SeqCst)
    }

    pub(super) fn set_connected_at(&self, at: u64) {
        self.connected_at.store(at, Ordering::SeqCst);
    }

    pub fn disconnected_at(&self) -> u64 {
        self.disconnected_at.load(Ordering::SeqCst)
    }

    pub(super) fn set_disconnected_at(&self, at: u64) {
        self.disconnected_at.store(at, Ordering::SeqCst);
    }

    pub fn alive(&self) -> u64 {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn update_alive(&self) {
        let connected_at =
            UNIX_EPOCH + Duration::from_secs(self.connected_at.load(Ordering::SeqCst));
        let alive = duration_since(SystemTime::now(), connected_at).as_secs();

        self.alive.store(alive, Ordering::SeqCst);
    }

    pub(super) fn set_alive(&self, live: u64) {
        self.alive.store(live, Ordering::SeqCst);
    }

    pub fn retry_ready(&self) -> bool {
        let next_attempt = Duration::from_secs(self.next_attempt.load(Ordering::SeqCst));

        Self::now() > next_attempt.as_secs()
    }

    pub fn retry(&self) -> u8 {
        self.retry.load(Ordering::SeqCst)
    }

    pub fn set_retry(&self, retry: u8) {
        self.retry.store(retry, Ordering::SeqCst);
        self.attempt_at.store(Self::now(), Ordering::SeqCst);

        let mut secs = BACKOFF_BASE.pow(retry as u32);
        if secs > MAX_RETRY_INTERVAL {
            secs = MAX_RETRY_INTERVAL;
        }

        let next_attempt = Self::now().saturating_add(secs);
        self.next_attempt.store(next_attempt, Ordering::SeqCst);
    }

    pub(super) fn set_next_attempt(&self, at: u64) {
        self.next_attempt.store(at, Ordering::SeqCst);
    }

    pub fn increase_retry(&self) {
        let last_attempt = UNIX_EPOCH + Duration::from_secs(self.attempt_at.load(Ordering::SeqCst));

        // Every time we try connect to a peer, we use all addresses. If
        // fail, we should only increase once.
        if duration_since(SystemTime::now(), last_attempt).as_secs() < VALID_ATTEMPT_INTERVAL {
            return;
        }

        let retry = self.retry.load(Ordering::SeqCst).saturating_add(1);
        self.set_retry(retry);
    }

    pub fn reset_retry(&self) {
        self.retry.store(0, Ordering::SeqCst);
    }

    pub fn pubkey_to_chain_addr(pubkey: &PublicKey) -> Result<Address, ErrorKind> {
        let pubkey_bytes = Bytes::from(pubkey.inner_ref().clone());

        Address::from_pubkey_bytes(pubkey_bytes.clone()).map_err(|e| ErrorKind::NoChainAddress {
            pubkey: pubkey_bytes,
            cause:  Box::new(e),
        })
    }

    pub fn mark_connected(&self, sid: SessionId) {
        self.set_connectedness(Connectedness::Connected);
        self.set_session_id(sid);
        self.reset_retry();
        self.update_connected();
    }

    pub fn mark_disconnected(&self) {
        self.set_connectedness(Connectedness::CanConnect);
        self.set_session_id(0.into());
        self.update_disconnected();
        self.update_alive();
    }

    fn update_connected(&self) {
        self.connected_at.store(Self::now(), Ordering::SeqCst);
    }

    fn update_disconnected(&self) {
        self.disconnected_at.store(Self::now(), Ordering::SeqCst);
    }

    fn now() -> u64 {
        duration_since(SystemTime::now(), UNIX_EPOCH).as_secs()
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} chain addr {:?} multiaddr {:?} last connected at {} alive {} retry {} current {}",
            self.id,
            self.chain_addr,
            self.multiaddrs.read().iter(),
            self.connected_at.load(Ordering::SeqCst),
            self.alive.load(Ordering::SeqCst),
            self.retry.load(Ordering::SeqCst),
            Connectedness::from(self.connectedness.load(Ordering::SeqCst))
        )
    }
}

#[derive(Debug, Display, Clone)]
#[display(fmt = "{}", _0)]
pub struct ArcPeer(Arc<Peer>);

impl ArcPeer {
    pub fn from_pubkey(pubkey: PublicKey) -> Result<Self, ErrorKind> {
        Ok(ArcPeer(Arc::new(Peer::from_pubkey(pubkey)?)))
    }
}

impl Deref for ArcPeer {
    type Target = Peer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<PeerId> for ArcPeer {
    fn borrow(&self) -> &PeerId {
        &self.id
    }
}

impl PartialEq for ArcPeer {
    fn eq(&self, other: &ArcPeer) -> bool {
        self.id == other.id
    }
}

impl Eq for ArcPeer {}

impl Hash for ArcPeer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

fn duration_since(now: SystemTime, early: SystemTime) -> Duration {
    match now.duration_since(early) {
        Ok(duration) => duration,
        Err(e) => e.duration(),
    }
}
