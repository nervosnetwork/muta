use crate::peer_manager::default_manager::{BorrowMutExt, PeerManagerHandle};
use crate::peer_manager::{ConnecStatus, PeerManager};
use crate::ping::{Behavior, PeerManager as PingPeerManager};

use tentacle::secio::PeerId;

// TODO: more score
impl<M: PeerManager> PingPeerManager for PeerManagerHandle<M> {
    fn update_peer_status(&mut self, peer_id: &PeerId, kind: Behavior) {
        let peer_mgr = self.borrow_mut::<M>();

        match kind {
            Behavior::Timeout => {
                peer_mgr.set_peer_status(peer_id, ConnecStatus::Disconnect);
                peer_mgr.update_peer_score(peer_id, -2);
            }
            Behavior::Ping => {
                peer_mgr.update_peer_score(peer_id, 1);
            }
            _ => (),
        }
    }
}
