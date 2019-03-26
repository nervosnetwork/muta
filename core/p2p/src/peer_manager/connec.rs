use crate::connec::{PeerManager as ConnecPeerManager, RemoteAddr};
use crate::peer_manager::default_manager::{BorrowExt, BorrowMutExt, PeerManagerHandle};
use crate::peer_manager::{ConnecStatus, PeerManager};

use tentacle::{multiaddr::Multiaddr, service::DialProtocol};

impl<M: PeerManager> ConnecPeerManager for PeerManagerHandle<M> {
    fn unconnected_multiaddrs(&mut self) -> Vec<RemoteAddr> {
        let peer_mgr = self.borrow::<M>();
        let max_peer_conns = M::max_peer_conns();

        // a masquer address is unconnected:
        // 1. doesn't have a peer id
        // 2. doesn't have relative connection status
        // 3. is currently disconnected
        // TODO: non-encrypt or 'always' connection fail should be
        // banned for a while.
        let filter_unconnected = |addr: Multiaddr| -> Option<RemoteAddr> {
            let remote_addr = RemoteAddr::new(addr.clone(), DialProtocol::All);

            peer_mgr
                .peer_id(&addr)
                .and_then(|peer_id| peer_mgr.connec_status(&peer_id))
                .and_then(|connec| match connec {
                    ConnecStatus::Disconnect => None,
                    _ => Some(()),
                })
                .map_or_else(|| Some(remote_addr), |_| None)
        };

        let remote_peers = peer_mgr.filter_random_masquer_addrs(max_peer_conns, filter_unconnected);

        // Update connection status to 'Connecting'
        let peer_mgr = self.borrow_mut::<M>();
        for peer in remote_peers.iter() {
            if let Some(peer_id) = peer_mgr.peer_id(peer.addr()) {
                peer_mgr.set_peer_status(&peer_id, ConnecStatus::Connecting)
            }
        }

        remote_peers
    }
}
