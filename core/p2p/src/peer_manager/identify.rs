use crate::identify::{MisbehaveResult, Misbehavior, PeerManager as IdentifyPeerManager};
use crate::peer_manager::default_manager::{BorrowMutExt, PeerManagerHandle};
use crate::peer_manager::{ConnecStatus, PeerManager};

use tentacle::service::SessionType;
use tentacle::{multiaddr::Multiaddr, secio::PeerId};

impl<M: PeerManager> IdentifyPeerManager for PeerManagerHandle<M> {
    /// Get local listen addresses
    fn local_listen_addrs(&mut self) -> Vec<Multiaddr> {
        let peer_mgr = self.borrow_mut::<M>();

        peer_mgr.local_listen_addrs().clone()
    }

    /// Add remote peer's listen addresses
    fn add_remote_listen_addrs(&mut self, peer_id: &PeerId, addrs: Vec<Multiaddr>) {
        let peer_mgr = self.borrow_mut::<M>();

        peer_mgr.new_peer(peer_id.clone(), addrs);
    }

    /// Add our address observed by remote peer
    fn add_observed_addr(
        &mut self,
        peer_id: &PeerId,
        addr: Multiaddr,
        _ty: SessionType,
    ) -> MisbehaveResult {
        let peer_mgr = self.borrow_mut::<M>();

        peer_mgr.new_peer(peer_id.clone(), vec![addr]);

        MisbehaveResult::Continue
    }

    /// Report misbehavior
    fn misbehave(&mut self, peer_id: &PeerId, _kind: Misbehavior) -> MisbehaveResult {
        let peer_mgr = self.borrow_mut::<M>();

        // FIXME: score system
        peer_mgr.set_peer_status(peer_id, ConnecStatus::Disconnect);

        MisbehaveResult::Disconnect
    }
}
