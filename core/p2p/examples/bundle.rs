#![feature(async_await, await_macro, futures_api)]

use peer_manager::DemoPeerManager;

use core_p2p::connec::ConnecProtocol;
use core_p2p::discovery::DiscoveryProtocol;
use core_p2p::identify::IdentifyProtocol;
use core_p2p::ping::PingProtocol;

use env_logger;
use futures03::compat::Stream01CompatExt;
use futures03::future::ready;
use futures03::prelude::StreamExt;
use log::{error, info, warn};
use tentacle::secio::{PeerId, SecioKeyPair};
use tentacle::service::{DialProtocol, ProtocolMeta, ServiceError, ServiceEvent};
use tentacle::{builder::ServiceBuilder, context::ServiceContext};
use tentacle::{multiaddr::Multiaddr, traits::ServiceHandle, ProtocolId};

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    env_logger::init();

    let (opt_server, listen_addr): (Option<Multiaddr>, Multiaddr) = {
        if std::env::args().nth(1) == Some("server".to_string()) {
            info!("Starting server .......");
            let listen_addr = "/ip4/127.0.0.1/tcp/1337".parse().unwrap();
            (None, listen_addr)
        } else {
            info!("Starting client ......");
            let port = std::env::args().nth(1).unwrap().parse::<u64>().unwrap();
            let server = "/ip4/127.0.0.1/tcp/1337".parse().unwrap();
            let listen_addr = format!("/ip4/127.0.0.1/tcp/{}", port).parse().unwrap();
            (Some(server), listen_addr)
        }
    };

    let key_pair = SecioKeyPair::secp256k1_generated();
    let peer_id = key_pair.to_peer_id();
    let (disc, identify, connec, ping) = build_protocols(1, peer_id, listen_addr.clone());
    let mut service = ServiceBuilder::default()
        .insert_protocol(identify)
        .insert_protocol(disc)
        .insert_protocol(connec)
        .insert_protocol(ping)
        .key_pair(key_pair)
        .forever(true)
        .build(SHandle {});

    opt_server.and_then(|server| {
        let _ = service.dial(server, DialProtocol::All);
        Some(())
    });
    let _ = service.listen(listen_addr);

    await!(service.compat().for_each(|_| ready(())))
}

fn build_protocols(
    initial_id: ProtocolId,
    peer_id: PeerId,
    listen_addr: Multiaddr,
) -> (ProtocolMeta, ProtocolMeta, ProtocolMeta, ProtocolMeta) {
    let mut peer_manager = DemoPeerManager::new();

    let disc = DiscoveryProtocol::build(initial_id, peer_manager.clone());
    let ident = IdentifyProtocol::build(initial_id + 1, peer_manager.clone());
    let connec = ConnecProtocol::build(initial_id + 2, peer_manager.clone());
    let ping = PingProtocol::build(initial_id + 3, peer_manager.clone());

    // Ourself should be known
    peer_manager.new_peer(&peer_id, vec![listen_addr]);

    (disc, ident, connec, ping)
}

struct SHandle {}

impl ServiceHandle for SHandle {
    fn handle_error(&mut self, _env: &mut ServiceContext, error: ServiceError) {
        error!("service error: {:?}", error);
    }

    fn handle_event(&mut self, _env: &mut ServiceContext, event: ServiceEvent) {
        info!("service event: {:?}", event);
    }
}

mod peer_manager {
    use parking_lot::RwLock;
    use rand::seq::IteratorRandom;
    use tentacle::multiaddr::Multiaddr;
    use tentacle::secio::PeerId;

    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    const MAX_CONNECTIONS: usize = 30;
    const INITIAL_SCORE: i32 = 100;
    const CONNECTED_NEW_ADDR_SCORE: i32 = 20;

    #[derive(Clone, Debug)]
    pub enum ConnecStatus {
        Connected,
        Connecting,
        Disconnect,
        // Banned, // TODO: implement Banned
    }

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

    pub struct DemoPeerManager {
        peers:   Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        connecs: Arc<RwLock<HashMap<PeerId, PeerConnec>>>,

        addr_peers: Arc<RwLock<HashMap<Multiaddr, PeerId>>>,
        // Multiaddrs from discovery
        masquer_addrs: Arc<RwLock<HashSet<Multiaddr>>>,
    }

    impl Clone for DemoPeerManager {
        fn clone(&self) -> Self {
            DemoPeerManager {
                peers:         Arc::clone(&self.peers),
                connecs:       Arc::clone(&self.connecs),
                addr_peers:    Arc::clone(&self.addr_peers),
                masquer_addrs: Arc::clone(&self.masquer_addrs),
            }
        }
    }

    impl DemoPeerManager {
        pub fn new() -> Self {
            DemoPeerManager {
                peers:         Default::default(),
                connecs:       Default::default(),
                addr_peers:    Default::default(),
                masquer_addrs: Default::default(),
            }
        }

        pub fn peer_id(&self, addr: &Multiaddr) -> Option<PeerId> {
            self.addr_peers.read().get(addr).map(Clone::clone)
        }

        pub fn connec_status(&self, peer_id: &PeerId) -> Option<ConnecStatus> {
            self.connecs.read().get(peer_id).map(|c| c.status.clone())
        }

        pub fn filter_random_masquer_addrs<B, F>(&self, n: usize, f: F) -> Vec<B>
        where
            F: Fn(Multiaddr) -> Option<B>,
        {
            let mut rng = rand::thread_rng();

            self.masquer_addrs
                .read()
                .iter()
                .choose_multiple(&mut rng, n)
                .iter()
                .filter_map(|addr| f((*addr).clone()))
                .collect()
        }

        pub fn random_masquer_addrs(&self, n: usize) -> Vec<Multiaddr> {
            self.filter_random_masquer_addrs(n, Some)
        }

        pub fn new_peer(&mut self, peer_id: &PeerId, addrs: Vec<Multiaddr>) {
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

        pub fn add_masquer_addr(&mut self, addr: Multiaddr) {
            self.masquer_addrs.write().insert(addr);
        }

        pub fn update_peer_score(&mut self, peer_id: &PeerId, score: Score) -> Score {
            let mut peer_info = self.peers.write();

            let info = peer_info
                .entry(peer_id.clone())
                .and_modify(|info| {
                    info.update_score(score);
                })
                .or_insert_with(PeerInfo::new);

            info.score
        }

        pub fn set_peer_status(&mut self, peer_id: &PeerId, status: ConnecStatus) {
            if let Some(connec) = self.connecs.write().get_mut(peer_id) {
                connec.set_status(status)
            }
        }
    }

    pub mod connec {
        use super::super::{warn, DialProtocol};
        use super::{ConnecStatus, DemoPeerManager, MAX_CONNECTIONS};
        use core_p2p::connec::{PeerManager, RemoteAddr};

        impl PeerManager for DemoPeerManager {
            fn unconnected_multiaddrs(&mut self) -> Vec<RemoteAddr> {
                let peer_mgr = self.clone();

                warn!("connec: addr set: {:?}", *self.masquer_addrs.read());
                warn!("connec: addr peer map: {:?}", *self.addr_peers.read());
                warn!("connec: peer connec: {:?}", *self.connecs.read());

                let remote_peers = self.filter_random_masquer_addrs(MAX_CONNECTIONS, |ref addr| {
                    let remote_peer = RemoteAddr::new(addr.clone(), DialProtocol::All);

                    // TODO: non-encrypt or 'always' connection fail should be
                    // banned for a while.
                    let opt_id = peer_mgr.peer_id(addr);
                    if None == opt_id {
                        return Some(remote_peer);
                    }

                    let peer_id = opt_id.unwrap();
                    let opt_connec = peer_mgr.connec_status(&peer_id);
                    if opt_connec.is_none() {
                        return Some(remote_peer);
                    }

                    let connec = opt_connec.unwrap();
                    match connec {
                        ConnecStatus::Disconnect => Some(remote_peer),
                        _ => None,
                    }
                });

                for peer in remote_peers.iter() {
                    if let Some(peer_id) = self.peer_id(peer.addr()) {
                        self.set_peer_status(&peer_id, ConnecStatus::Connecting)
                    }
                }

                warn!("connec: new peers {:?}", remote_peers);

                remote_peers
            }
        }
    }

    pub mod identify {
        use super::super::{warn, Multiaddr, PeerId};
        use super::{ConnecStatus, DemoPeerManager};
        use core_p2p::identify::{MisbehaveResult, Misbehavior, PeerManager};

        impl PeerManager for DemoPeerManager {
            fn add_listen_addrs(&mut self, peer_id: &PeerId, addrs: Vec<Multiaddr>) {
                warn!("identify: add listen addrs: {:?}", addrs);

                self.new_peer(peer_id, addrs);
            }

            fn add_observed_addr(&mut self, peer_id: &PeerId, addr: Multiaddr) -> MisbehaveResult {
                warn!("identify: add observed addr: {:?}", addr);

                self.new_peer(peer_id, vec![addr]);

                MisbehaveResult::Continue
            }

            /// Report misbehavior
            fn misbehave(&mut self, peer_id: &PeerId, _kind: Misbehavior) -> MisbehaveResult {
                // FIXME: score system
                self.set_peer_status(peer_id, ConnecStatus::Disconnect);

                MisbehaveResult::Disconnect
            }
        }
    }

    pub mod discovery {
        use super::super::{warn, Multiaddr};
        use super::DemoPeerManager;
        use core_p2p::discovery::{MisbehaveResult, Misbehavior, PeerManager};

        impl PeerManager for DemoPeerManager {
            fn add_new(&mut self, addr: Multiaddr) {
                warn!("disc: add new multiaddr: {:?}", addr);

                self.add_masquer_addr(addr);

                warn!("disc: addr set: {:?}", self.masquer_addrs);
            }

            fn misbehave(&mut self, addr: Multiaddr, _kind: Misbehavior) -> MisbehaveResult {
                warn!("disc: {} misbehave", addr);

                // TODO: have score based on masquer addresses?
                let peer_id = self.peer_id(&addr).unwrap();
                let score = self.update_peer_score(&peer_id, -20);

                if score < 0 {
                    return MisbehaveResult::Disconnect;
                }
                MisbehaveResult::Continue
            }

            // TODO: include peers with good score?
            fn get_random(&mut self, n: usize) -> Vec<Multiaddr> {
                self.random_masquer_addrs(n)
            }
        }
    }

    pub mod ping {
        use super::super::{warn, PeerId};
        use super::{ConnecStatus, DemoPeerManager};
        use core_p2p::ping::{Behavior, PeerManager};

        impl PeerManager for DemoPeerManager {
            fn update_peer_status(&mut self, peer_id: &PeerId, kind: Behavior) {
                warn!("update_peer_status: {:?}", kind);

                match kind {
                    Behavior::Timeout => {
                        self.set_peer_status(peer_id, ConnecStatus::Disconnect);
                        self.update_peer_score(peer_id, -2);
                    }
                    Behavior::Ping => {
                        self.update_peer_score(peer_id, 1);
                    }
                    _ => (),
                }
            }
        }
    }

}
