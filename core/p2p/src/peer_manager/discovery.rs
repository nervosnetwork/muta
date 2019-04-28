use crate::discovery::{MisbehaveResult, Misbehavior, PeerManager as DiscoveryPeerManager};
use crate::peer_manager::{
    default_manager::{BorrowMutExt, PeerManagerHandle},
    PeerManager,
};

use tentacle::multiaddr::Multiaddr;
use tentacle::SessionId;

impl<M: PeerManager> DiscoveryPeerManager for PeerManagerHandle<M> {
    fn add_new_addr(&mut self, _session_id: SessionId, addr: Multiaddr) {
        let peer_mgr = self.borrow_mut::<M>();

        peer_mgr.new_masquer_addr(addr);
    }

    fn add_new_addrs(&mut self, session_id: SessionId, addrs: Vec<Multiaddr>) {
        for addr in addrs.into_iter() {
            self.add_new_addr(session_id, addr)
        }
    }

    fn misbehave(&mut self, _session_id: SessionId, _kind: Misbehavior) -> MisbehaveResult {
        // TODO: Has not handled misbehave.
        MisbehaveResult::Disconnect
        // let peer_mgr = self.borrow_mut::<M>();

        // // TODO: have score based on masquer addresses?
        // let score = {
        //     if let Some(peer_id) = peer_mgr.peer_id(&addr) {
        //         peer_mgr.update_peer_score(&peer_id, -20)
        //     } else {
        //         // a connected peer must have relative peer id, if not,
        //         // disconnected it.
        //         -1
        //     }
        // };

        // if score > 0 {
        //     MisbehaveResult::Continue
        // } else {
        //     MisbehaveResult::Disconnect
        // }
    }

    fn get_random(&mut self, n: usize) -> Vec<Multiaddr> {
        let peer_mgr = self.borrow_mut::<M>();

        peer_mgr.random_masquer_addrs(n)
    }
}
