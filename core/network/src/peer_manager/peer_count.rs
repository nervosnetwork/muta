use core_runtime::network;

use crate::peer_manager::PeerManager;

pub struct PeerCount<M> {
    peer_mgr: M,
}

impl<M> PeerCount<M> {
    pub fn new(peer_mgr: M) -> Self {
        PeerCount { peer_mgr }
    }
}

impl<M> network::PeerCount for PeerCount<M>
where
    M: PeerManager + Send + Sync,
{
    fn peer_count(&self) -> usize {
        self.peer_mgr.connected_count()
    }
}
