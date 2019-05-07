use crate::peer_manager::{ConnecStatus, PeerManager};

use parking_lot::RwLock;
use rand::seq::IteratorRandom;
use tentacle::multiaddr::Multiaddr;
use tentacle::secio::PeerId;

use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::sync::Arc;

const INITIAL_SCORE: i32 = 100;
const CONNECTED_NEW_ADDR_SCORE: i32 = 20;

#[derive(Clone, Debug)]
pub struct PeerConnec {
    addrs: Vec<Multiaddr>,
    // session: Option<SessionContext>,
    status: ConnecStatus,
}

impl PeerConnec {
    pub fn from_status(status: ConnecStatus) -> Self {
        PeerConnec {
            addrs: Default::default(),
            status,
        }
    }

    pub fn add_multiaddrs(&mut self, addrs: Vec<Multiaddr>) {
        self.addrs.extend(addrs)
    }

    pub fn set_status(&mut self, status: ConnecStatus) {
        self.status = status
    }
}

impl Default for PeerConnec {
    fn default() -> Self {
        PeerConnec {
            addrs:  Default::default(),
            status: ConnecStatus::Disconnect,
        }
    }
}

pub type Score = i32;

#[derive(Clone)]
pub struct PeerInfo {
    score: Score,
    // _ban_expired: u64, FIXME: ban support
}

impl PeerInfo {
    pub fn new() -> Self {
        PeerInfo {
            score: INITIAL_SCORE,
        }
    }

    pub fn update_score(&mut self, score: Score) -> Score {
        if score > 0 {
            self.score.saturating_add(score)
        } else {
            self.score.saturating_sub(-score)
        }
    }
}

pub struct DefaultPeerManagerImpl {
    peers:   Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    connecs: Arc<RwLock<HashMap<PeerId, PeerConnec>>>,

    addr_peers: Arc<RwLock<HashMap<Multiaddr, PeerId>>>,
    // Multiaddrs from discovery
    masquer_addrs: Arc<RwLock<HashSet<Multiaddr>>>,

    // Ourselves
    pub(crate) local_listen_addrs: Arc<RwLock<HashSet<Multiaddr>>>,
}

impl DefaultPeerManagerImpl {
    pub fn new() -> Self {
        DefaultPeerManagerImpl {
            peers:              Default::default(),
            connecs:            Default::default(),
            addr_peers:         Default::default(),
            masquer_addrs:      Default::default(),
            local_listen_addrs: Default::default(),
        }
    }
}

impl Default for DefaultPeerManagerImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DefaultPeerManagerImpl {
    fn clone(&self) -> Self {
        DefaultPeerManagerImpl {
            peers:              Arc::clone(&self.peers),
            connecs:            Arc::clone(&self.connecs),
            addr_peers:         Arc::clone(&self.addr_peers),
            masquer_addrs:      Arc::clone(&self.masquer_addrs),
            local_listen_addrs: Arc::clone(&self.local_listen_addrs),
        }
    }
}

impl PeerManager for DefaultPeerManagerImpl {
    fn local_listen_addrs(&mut self) -> Vec<Multiaddr> {
        self.local_listen_addrs
            .read()
            .iter()
            .map(Clone::clone)
            .collect::<Vec<Multiaddr>>()
    }

    fn peer_id(&self, addr: &Multiaddr) -> Option<PeerId> {
        self.addr_peers.read().get(addr).map(Clone::clone)
    }

    fn connec_status(&self, peer_id: &PeerId) -> Option<ConnecStatus> {
        self.connecs.read().get(peer_id).map(|c| c.status.clone())
    }

    fn filter_random_masquer_addrs<T, F>(&self, n: usize, f: F) -> Vec<T>
    where
        F: Fn(Multiaddr) -> Option<T>,
    {
        let mut rng = rand::thread_rng();

        self.masquer_addrs
            .read()
            .iter()
            .choose_multiple(&mut rng, n)
            .into_iter()
            .filter_map(|addr| f((*addr).clone()))
            .collect()
    }

    fn random_masquer_addrs(&self, n: usize) -> Vec<Multiaddr> {
        self.filter_random_masquer_addrs(n, Some)
    }

    fn new_peer(&mut self, peer_id: PeerId, addrs: Vec<Multiaddr>) {
        self.peers
            .write()
            .entry(peer_id.clone())
            .and_modify(|info| {
                info.update_score(CONNECTED_NEW_ADDR_SCORE);
            })
            .or_insert_with(PeerInfo::new);

        self.connecs
            .write()
            .entry(peer_id.clone())
            .and_modify(|connec| {
                connec.add_multiaddrs(addrs.clone());
                connec.set_status(ConnecStatus::Connected)
            })
            .or_insert_with(|| PeerConnec::from_status(ConnecStatus::Connected));

        for addr in addrs {
            self.addr_peers
                .write()
                .insert(addr.clone(), peer_id.clone());
        }
    }

    fn new_masquer_addr(&mut self, addr: Multiaddr) {
        self.masquer_addrs.write().insert(addr);
    }

    fn update_peer_score(&mut self, peer_id: &PeerId, score: Score) -> Score {
        let mut peer_info = self.peers.write();

        let info = peer_info
            .entry(peer_id.clone())
            .and_modify(|info| {
                info.update_score(score);
            })
            .or_insert_with(PeerInfo::new);

        info.score
    }

    fn set_peer_status(&mut self, peer_id: &PeerId, status: ConnecStatus) {
        if let Some(connec) = self.connecs.write().get_mut(peer_id) {
            connec.set_status(status)
        }
    }
}
