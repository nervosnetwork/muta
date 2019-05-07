pub mod default_manager;

pub(crate) mod connec;
pub(crate) mod default_impl;
pub(crate) mod discovery;
pub(crate) mod identify;
pub(crate) mod ping;
pub(crate) mod transmission;

pub(crate) use default_impl::DefaultPeerManagerImpl;
pub use default_manager::DefaultPeerManager;

use tentacle::multiaddr::Multiaddr;
use tentacle::secio::PeerId;

const MAX_PEER_CONNECTIONS: usize = 30;

pub type Score = i32;

#[derive(Clone, Debug)]
pub enum ConnecStatus {
    Connected,
    Connecting,
    Disconnect,
    Banned,
}

pub trait PeerManager: Send + Sync + Clone {
    fn local_listen_addrs(&mut self) -> Vec<Multiaddr>;

    fn max_peer_conns() -> usize {
        MAX_PEER_CONNECTIONS
    }

    fn peer_id(&self, addr: &Multiaddr) -> Option<PeerId>;

    fn connec_status(&self, peer_id: &PeerId) -> Option<ConnecStatus>;

    fn filter_random_masquer_addrs<U, F>(&self, n: usize, f: F) -> Vec<U>
    where
        F: Fn(Multiaddr) -> Option<U>;

    fn random_masquer_addrs(&self, n: usize) -> Vec<Multiaddr> {
        self.filter_random_masquer_addrs(n, Some)
    }

    fn new_peer(&mut self, peer_id: PeerId, addrs: Vec<Multiaddr>);

    fn new_masquer_addr(&mut self, addr: Multiaddr);

    fn remove_masquer_addr(&mut self, addr: &Multiaddr);

    fn update_peer_score(&mut self, peer_id: &PeerId, score: Score) -> Score;

    fn set_peer_status(&mut self, peer_id: &PeerId, status: ConnecStatus);
}
