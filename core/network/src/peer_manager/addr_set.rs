use super::{PeerMultiaddr, MAX_RETRY_COUNT};

use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

use parking_lot::RwLock;
use tentacle::{multiaddr::Multiaddr, secio::PeerId};

use crate::traits::MultiaddrExt;

const MAX_ADDR_FAILURE: u8 = MAX_RETRY_COUNT;

#[derive(Debug)]
struct AddrInfo {
    addr:    PeerMultiaddr,
    failure: AtomicUsize,
}

impl AddrInfo {
    pub fn owned_addr(&self) -> PeerMultiaddr {
        self.addr.to_owned()
    }

    pub fn owned_raw_addr(&self) -> Multiaddr {
        (*self.addr).to_owned()
    }

    #[cfg(test)]
    pub fn failure(&self) -> usize {
        self.failure.load(Ordering::SeqCst)
    }

    pub fn inc_failure(&self) {
        self.failure.fetch_add(1, Ordering::SeqCst);
    }

    pub fn give_up(&self) {
        self.failure
            .store(MAX_ADDR_FAILURE as usize + 1, Ordering::SeqCst);
    }

    pub fn reset_failure(&self) {
        self.failure.store(0, Ordering::SeqCst);
    }

    pub fn connectable(&self) -> bool {
        self.failure.load(Ordering::SeqCst) <= MAX_ADDR_FAILURE as usize
    }
}

impl Deref for AddrInfo {
    type Target = PeerMultiaddr;

    fn deref(&self) -> &Self::Target {
        &self.addr
    }
}

impl From<PeerMultiaddr> for AddrInfo {
    fn from(pma: PeerMultiaddr) -> AddrInfo {
        AddrInfo {
            addr:    pma,
            failure: AtomicUsize::new(0),
        }
    }
}

impl Borrow<PeerMultiaddr> for AddrInfo {
    fn borrow(&self) -> &PeerMultiaddr {
        &self.addr
    }
}

impl PartialEq for AddrInfo {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}

impl Eq for AddrInfo {}

impl Hash for AddrInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.addr.hash(state)
    }
}

#[derive(Debug)]
pub struct PeerAddrSet {
    peer_id: PeerId,
    inner:   RwLock<HashSet<AddrInfo>>,
}

impl PeerAddrSet {
    pub fn new(peer_id: PeerId) -> Self {
        PeerAddrSet {
            peer_id,
            inner: Default::default(),
        }
    }

    pub fn insert(&self, multiaddrs: Vec<PeerMultiaddr>) {
        let multiaddrs = {
            let set = self.inner.read();

            // Filter already exists multiaddrs, we dont reset failure.
            multiaddrs
                .into_iter()
                .filter(|pma| self.match_peer_id(&pma) && !set.contains(pma))
                .map(Into::into)
                .collect::<HashSet<_>>()
        };

        self.inner.write().extend(multiaddrs);
    }

    pub fn set(&self, multiaddrs: Vec<PeerMultiaddr>) {
        let multiaddrs = multiaddrs
            .into_iter()
            .filter(|pma| self.match_peer_id(&pma))
            .map(Into::into)
            .collect::<HashSet<_>>();

        *self.inner.write() = multiaddrs;
    }

    pub(crate) fn insert_raw(&self, multiaddr: Multiaddr) {
        if let Some(id_bytes) = multiaddr.id_bytes() {
            if id_bytes != self.peer_id.as_bytes() {
                return;
            }
        }

        self.insert(vec![PeerMultiaddr::new(multiaddr, &self.peer_id)]);
    }

    pub fn remove(&self, multiaddr: &PeerMultiaddr) {
        self.inner.write().remove(multiaddr);
    }

    pub fn contains(&self, multiaddr: &PeerMultiaddr) -> bool {
        self.inner.read().contains(multiaddr)
    }

    pub fn all(&self) -> Vec<PeerMultiaddr> {
        self.inner.read().iter().map(AddrInfo::owned_addr).collect()
    }

    pub fn all_raw(&self) -> Vec<Multiaddr> {
        self.inner
            .read()
            .iter()
            .map(AddrInfo::owned_raw_addr)
            .collect()
    }

    pub fn connectable(&self) -> Vec<PeerMultiaddr> {
        let to_pma = |a: &'_ AddrInfo| -> Option<PeerMultiaddr> {
            if a.connectable() {
                Some(a.owned_addr())
            } else {
                None
            }
        };

        self.inner.read().iter().filter_map(to_pma).collect()
    }

    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    pub fn connectable_len(&self) -> usize {
        self.inner.read().iter().filter(|a| a.connectable()).count()
    }

    #[cfg(test)]
    pub fn failure(&self, pma: &PeerMultiaddr) -> Option<usize> {
        self.inner.read().get(pma).map(|a| a.failure())
    }

    pub fn inc_failure(&self, pma: &PeerMultiaddr) {
        if let Some(info) = self.inner.read().get(pma) {
            info.inc_failure();
        }
    }

    pub fn give_up(&self, pma: &PeerMultiaddr) {
        if let Some(info) = self.inner.read().get(pma) {
            info.give_up();
        }
    }

    pub fn reset_failure(&self, pma: &PeerMultiaddr) {
        if let Some(info) = self.inner.read().get(pma) {
            info.reset_failure();
        }
    }

    fn match_peer_id(&self, pma: &PeerMultiaddr) -> bool {
        pma.has_id() && pma.id_bytes() == Some(Cow::Borrowed(self.peer_id.as_bytes()))
    }
}
