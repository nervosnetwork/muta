use crate::discovery::{MisbehaveResult, Misbehavior, PeerManager as DiscoveryPeerManager};
use crate::peer_manager::{
    default_manager::{BorrowMutExt, PeerManagerHandle},
    PeerManager,
};

use tentacle::multiaddr::Multiaddr;

impl<M: PeerManager> DiscoveryPeerManager for PeerManagerHandle<M> {
    fn add_new(&mut self, addr: Multiaddr) {
        let peer_mgr = self.borrow_mut::<M>();

        peer_mgr.new_masquer_addr(addr);
    }

    fn misbehave(&mut self, addr: Multiaddr, _kind: Misbehavior) -> MisbehaveResult {
        let peer_mgr = self.borrow_mut::<M>();

        // TODO: have score based on masquer addresses?
        let score = {
            if let Some(peer_id) = peer_mgr.peer_id(&addr) {
                peer_mgr.update_peer_score(&peer_id, -20)
            } else {
                // a connected peer must have relative peer id, if not,
                // disconnected it.
                -1
            }
        };

        if score > 0 {
            MisbehaveResult::Continue
        } else {
            MisbehaveResult::Disconnect
        }
    }

    // TODO: include peers with good score?
    fn get_random(&mut self, n: usize) -> Vec<Multiaddr> {
        let peer_mgr = self.borrow_mut::<M>();

        peer_mgr.random_masquer_addrs(n)
    }
}
