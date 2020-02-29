use super::{PeerAddrSet, PeerMultiaddr, Retry, MAX_UNKNOWN_RETRY};

use std::{
    borrow::Borrow,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::Arc,
};

use tentacle::secio::PeerId;

#[derive(Debug)]
pub struct UnknownPeer {
    pub id:         Arc<PeerId>,
    pub multiaddrs: Arc<PeerAddrSet>,
    pub retry:      Retry,
}

#[derive(Debug, Clone)]
pub struct ArcUnknownPeer(Arc<UnknownPeer>);

impl UnknownPeer {
    pub fn owned_id(&self) -> PeerId {
        self.id.as_ref().to_owned()
    }
}

impl From<PeerMultiaddr> for ArcUnknownPeer {
    fn from(pma: PeerMultiaddr) -> ArcUnknownPeer {
        let peer_id = pma.peer_id();
        let addr_set = PeerAddrSet::new(peer_id.clone());
        addr_set.insert(vec![pma]);

        let peer = UnknownPeer {
            id:         Arc::new(peer_id),
            multiaddrs: Arc::new(addr_set),
            retry:      Retry::new(MAX_UNKNOWN_RETRY),
        };

        ArcUnknownPeer(Arc::new(peer))
    }
}

impl Borrow<PeerId> for ArcUnknownPeer {
    fn borrow(&self) -> &PeerId {
        &self.id.as_ref()
    }
}

impl PartialEq for ArcUnknownPeer {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ArcUnknownPeer {}

impl Hash for ArcUnknownPeer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl Deref for ArcUnknownPeer {
    type Target = UnknownPeer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
