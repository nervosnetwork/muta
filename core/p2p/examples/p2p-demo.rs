use core_p2p::connec::ConnecProtocol;
use core_p2p::discovery::DiscoveryProtocol;
use core_p2p::identify::IdentifyProtocol;
use core_p2p::peer_manager::DefaultPeerManager;
use core_p2p::ping::PingProtocol;

use env_logger;
use futures::prelude::Stream;
use log::{error, info};
use tentacle::secio::{PeerId, SecioKeyPair};
use tentacle::service::{DialProtocol, ProtocolMeta, ServiceError, ServiceEvent};
use tentacle::{builder::ServiceBuilder, context::ServiceContext};
use tentacle::{multiaddr::Multiaddr, traits::ServiceHandle, ProtocolId};

fn main() {
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
    tokio::run(service.for_each(|_| Ok(())))
}

fn build_protocols(
    initial_id: ProtocolId,
    peer_id: PeerId,
    listen_addr: Multiaddr,
) -> (ProtocolMeta, ProtocolMeta, ProtocolMeta, ProtocolMeta) {
    let mut peer_manager = DefaultPeerManager::new();

    let disc = DiscoveryProtocol::build(initial_id, peer_manager.clone());
    let ident = IdentifyProtocol::build(initial_id + 1, peer_manager.clone());
    let connec = ConnecProtocol::build(initial_id + 2, peer_manager.clone());
    let ping = PingProtocol::build(initial_id + 3, peer_manager.clone());

    // Ourself should be known
    peer_manager.register_self(peer_id, vec![listen_addr]);

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
