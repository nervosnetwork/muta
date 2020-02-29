use super::{time, ArcUnknownPeer, PeerAddrSet, Retry, MAX_RETRY_COUNT};

use std::{
    borrow::Borrow,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use derive_more::Display;
use protocol::{types::Address, Bytes};
use tentacle::{
    secio::{PeerId, PublicKey},
    SessionId,
};

use crate::error::ErrorKind;

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
    pub chain_addr:  Arc<Address>,
    pub multiaddrs:  Arc<PeerAddrSet>,
    pub retry:       Retry,
    connectedness:   AtomicUsize,
    session_id:      AtomicUsize,
    connected_at:    AtomicU64,
    disconnected_at: AtomicU64,
    alive:           AtomicU64,
}

impl Peer {
    pub fn from_pubkey(pubkey: PublicKey) -> Result<Self, ErrorKind> {
        let chain_addr = Peer::pubkey_to_chain_addr(&pubkey)?;
        let peer_id = pubkey.peer_id();

        let peer = Peer {
            id:              Arc::new(peer_id.clone()),
            pubkey:          Arc::new(pubkey),
            multiaddrs:      Arc::new(PeerAddrSet::new(peer_id)),
            chain_addr:      Arc::new(chain_addr),
            retry:           Retry::new(MAX_RETRY_COUNT),
            connectedness:   AtomicUsize::new(Connectedness::CanConnect as usize),
            session_id:      AtomicUsize::new(0),
            connected_at:    AtomicU64::new(0),
            disconnected_at: AtomicU64::new(0),
            alive:           AtomicU64::new(0),
        };

        Ok(peer)
    }

    pub fn owned_id(&self) -> PeerId {
        self.id.as_ref().to_owned()
    }

    pub fn owned_pubkey(&self) -> PublicKey {
        self.pubkey.as_ref().to_owned()
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
        let alive = time::duration_since(SystemTime::now(), connected_at).as_secs();

        self.alive.store(alive, Ordering::SeqCst);
    }

    pub(super) fn set_alive(&self, live: u64) {
        self.alive.store(live, Ordering::SeqCst);
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
        self.retry.reset();
        self.update_connected();
    }

    pub fn mark_disconnected(&self) {
        self.set_connectedness(Connectedness::CanConnect);
        self.set_session_id(0.into());
        self.update_disconnected();
        self.update_alive();
    }

    fn update_connected(&self) {
        self.connected_at.store(time::now(), Ordering::SeqCst);
    }

    fn update_disconnected(&self) {
        self.disconnected_at.store(time::now(), Ordering::SeqCst);
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
            self.retry.count(),
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

    pub fn from_unknown(unknown: ArcUnknownPeer, pubkey: PublicKey) -> Result<Self, ErrorKind> {
        let mut peer = Peer::from_pubkey(pubkey)?;
        peer.multiaddrs = Arc::clone(&unknown.multiaddrs);
        Ok(ArcPeer(Arc::new(peer)))
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
