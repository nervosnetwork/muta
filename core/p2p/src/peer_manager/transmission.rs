use crate::peer_manager::default_manager::{BorrowMutExt, PeerManagerHandle};
use crate::peer_manager::PeerManager;
use crate::transmission::{Misbehavior, MisbehaviorResult, PeerManager as TransmissionPeerManager};

use tentacle::{multiaddr::Multiaddr, secio::PeerId};

impl<M: PeerManager> TransmissionPeerManager for PeerManagerHandle<M> {
    fn misbehave(
        &mut self,
        peer_id: Option<PeerId>,
        addr: Multiaddr,
        _kind: Misbehavior,
    ) -> MisbehaviorResult {
        let peer_mgr = self.borrow_mut::<M>();

        let score = peer_id
            .or_else(|| peer_mgr.peer_id(&addr))
            .map_or_else(|| -1, |peer_id| peer_mgr.update_peer_score(&peer_id, -10));

        if score > 0 {
            MisbehaviorResult::Continue
        } else {
            MisbehaviorResult::Disconnect
        }
    }
}
