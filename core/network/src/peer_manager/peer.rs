use super::{time, PeerAddrSet, Retry, TrustMetric, MAX_RETRY_COUNT};

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
use parking_lot::RwLock;
use protocol::{types::Address, Bytes};
use tentacle::{
    secio::{PeerId, PublicKey},
    SessionId,
};

use crate::error::ErrorKind;

const CONNECTEDNESS_MASK: usize = 0b1110;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Display)]
#[repr(usize)]
pub enum Connectedness {
    #[display(fmt = "not connected")]
    NotConnected = 0 << 1,

    #[display(fmt = "can connect")]
    CanConnect = 1 << 1,

    #[display(fmt = "connected")]
    Connected = 2 << 1,

    #[display(fmt = "unconnectable")]
    Unconnectable = 3 << 1,

    #[display(fmt = "connecting")]
    Connecting = 4 << 1,
}

impl From<usize> for Connectedness {
    fn from(src: usize) -> Connectedness {
        use self::Connectedness::*;

        debug_assert!(
            src == NotConnected as usize
                || src == CanConnect as usize
                || src == Connected as usize
                || src == Unconnectable as usize
                || src == Connecting as usize
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
    pub id:          PeerId,
    pub multiaddrs:  PeerAddrSet,
    pub retry:       Retry,
    pubkey:          RwLock<Option<PublicKey>>,
    chain_addr:      RwLock<Option<Address>>,
    trust_metric:    RwLock<Option<TrustMetric>>,
    connectedness:   AtomicUsize,
    session_id:      AtomicUsize,
    connected_at:    AtomicU64,
    disconnected_at: AtomicU64,
    alive:           AtomicU64,
    ban_expired_at:  AtomicU64,
}

impl Peer {
    pub fn new(peer_id: PeerId) -> Self {
        Peer {
            id:              peer_id.clone(),
            multiaddrs:      PeerAddrSet::new(peer_id),
            retry:           Retry::new(MAX_RETRY_COUNT),
            pubkey:          RwLock::new(None),
            chain_addr:      RwLock::new(None),
            trust_metric:    RwLock::new(None),
            connectedness:   AtomicUsize::new(Connectedness::NotConnected as usize),
            session_id:      AtomicUsize::new(0),
            connected_at:    AtomicU64::new(0),
            disconnected_at: AtomicU64::new(0),
            alive:           AtomicU64::new(0),
            ban_expired_at:  AtomicU64::new(0),
        }
    }

    pub fn from_pubkey(pubkey: PublicKey) -> Result<Self, ErrorKind> {
        let peer = Peer::new(pubkey.peer_id());
        peer.set_pubkey(pubkey)?;

        Ok(peer)
    }

    pub fn owned_id(&self) -> PeerId {
        self.id.to_owned()
    }

    pub fn has_pubkey(&self) -> bool {
        self.pubkey.read().is_some()
    }

    pub fn owned_pubkey(&self) -> Option<PublicKey> {
        self.pubkey.read().clone()
    }

    pub fn owned_chain_addr(&self) -> Option<Address> {
        self.chain_addr.read().clone()
    }

    pub fn set_pubkey(&self, pubkey: PublicKey) -> Result<(), ErrorKind> {
        if pubkey.peer_id() != self.id {
            Err(ErrorKind::PublicKeyNotMatchId {
                pubkey,
                id: self.id.clone(),
            })
        } else {
            let chain_addr = Peer::pubkey_to_chain_addr(&pubkey)?;

            *self.pubkey.write() = Some(pubkey);
            *self.chain_addr.write() = Some(chain_addr);
            Ok(())
        }
    }

    pub fn trust_metric(&self) -> Option<TrustMetric> {
        self.trust_metric.read().clone()
    }

    pub fn set_trust_metric(&self, metric: TrustMetric) {
        *self.trust_metric.write() = Some(metric);
    }

    #[cfg(test)]
    pub fn remove_trust_metric(&self) {
        *self.trust_metric.write() = None;
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

    pub fn ban(&self, timeout: Duration) {
        let expired_at = Duration::from_secs(time::now()) + timeout;
        self.ban_expired_at
            .store(expired_at.as_secs(), Ordering::SeqCst);
    }

    #[cfg(test)]
    pub fn ban_expired_at(&self) -> u64 {
        self.ban_expired_at.load(Ordering::SeqCst)
    }

    pub fn banned(&self) -> bool {
        let expired_at = self.ban_expired_at.load(Ordering::SeqCst);
        if time::now() > expired_at {
            if expired_at > 0 {
                self.ban_expired_at.store(0, Ordering::SeqCst);
                if let Some(trust_metric) = self.trust_metric() {
                    // TODO: Reset just in case, may remove in
                    // the future.
                    trust_metric.reset_history();
                }
            }
            false
        } else {
            true
        }
    }

    #[cfg(test)]
    fn set_ban_expired_at(&self, at: u64) {
        self.ban_expired_at.store(at, Ordering::SeqCst);
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
            self.multiaddrs.all(),
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
    pub fn new(peer_id: PeerId) -> Self {
        ArcPeer(Arc::new(Peer::new(peer_id)))
    }

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

#[cfg(test)]
mod tests {
    use super::ArcPeer;
    use crate::peer_manager::{time, TrustMetric, TrustMetricConfig};

    use tentacle::secio::SecioKeyPair;

    use std::sync::Arc;

    #[test]
    fn should_reset_trust_metric_history_after_unban() {
        let keypair = SecioKeyPair::secp256k1_generated();
        let pubkey = keypair.public_key();
        let peer = ArcPeer::from_pubkey(pubkey).expect("make peer");
        let peer_trust_config = Arc::new(TrustMetricConfig::default());

        let trust_metric = TrustMetric::new(Arc::clone(&peer_trust_config));
        peer.set_trust_metric(trust_metric.clone());
        for _ in 0..2 {
            trust_metric.bad_events(10);
            trust_metric.enter_new_interval();
        }
        assert!(trust_metric.trust_score() < 40, "should lower score");

        peer.set_ban_expired_at(time::now() - 20);
        assert!(!peer.banned(), "should unban");

        assert_eq!(
            trust_metric.intervals(),
            0,
            "should reset peer trust history"
        );
    }
}
