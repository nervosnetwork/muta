use core_p2p::transmission::{CastMessage, Misbehavior, PeerManager, TransmissionProtocol};

use env_logger;
use futures::future::Future;
use futures::stream::Stream;
use log::{error, info};
use parking_lot::RwLock;
use tentacle::{
    builder::ServiceBuilder,
    context::ServiceContext,
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey, SecioKeyPair},
    service::{DialProtocol, Service, ServiceError, ServiceEvent},
    traits::ServiceHandle,
    ProtocolId,
};
use tokio;
use tokio::timer::Interval;

use std::clone::Clone;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const DEMO_PROTOCOL_ID: ProtocolId = 2077;

#[cfg(feature = "prost-message")]
mod message {
    use prost_derive::*;

    #[derive(Clone, PartialEq, Message)]
    pub struct Hello {
        #[prost(uint64, tag = "1")]
        pub id: u64,
        #[prost(string, tag = "2")]
        pub msg: String,
    }
}

#[cfg(not(feature = "prost-message"))]
mod message {
    use bytes::Bytes;
    use core_p2p::transmission::{Codec, RawMessage};

    #[derive(Debug)]
    pub struct Hello {
        pub id: u64,
        pub msg: String,
    }

    impl Codec for Hello {
        fn encode(self) -> RawMessage {
            Bytes::from(format!("{}: {}", self.id, self.msg))
        }

        fn decode(raw: &[u8]) -> Result<Hello, ()> {
            let msg = String::from_utf8(raw.to_owned()).map_err(|_| ())?;
            let parts: Vec<&str> = msg.split(':').collect();

            if 2 != parts.len() {
                return Err(());
            }

            let id = parts[0].parse::<u64>().map_err(|_| ())?;
            let msg = parts[1].trim().to_owned();

            Ok(Hello { id, msg })
        }
    }
}

type Score = i32;

struct DemoPeerManager {
    addrs: Arc<RwLock<HashMap<Multiaddr, Score>>>,
}

impl PeerManager for DemoPeerManager {
    fn misbehave(&mut self, _: Option<PeerId>, multiaddr: Multiaddr, _kind: Misbehavior) -> Score {
        let mut addrs = self.addrs.write();
        let value = addrs.entry(multiaddr).or_insert(100);
        *value -= 20;
        *value
    }
}

impl Clone for DemoPeerManager {
    fn clone(&self) -> Self {
        DemoPeerManager {
            addrs: Arc::clone(&self.addrs),
        }
    }
}

struct DemoService {
    // Local peer public key
    _pub_key: Option<PublicKey>,
}

impl ServiceHandle for DemoService {
    fn handle_error(&mut self, _control: &mut ServiceContext, error: ServiceError) {
        error!("Demo service error: {:?}", error);
    }

    fn handle_event(&mut self, control: &mut ServiceContext, event: ServiceEvent) {
        info!("Demo service event: {:?}", event);

        match event {
            ServiceEvent::SessionOpen {
                session_context: session,
            } if session.remote_pubkey.is_none() => {
                info!("Demo service: drop un-encypt session {}", session.id);
                control.disconnect(session.id);
            }
            ServiceEvent::SessionClose {
                session_context: session,
            } => {
                info!("Demo service: session {} disconnected", session.id);
            }
            ServiceEvent::SessionOpen { .. } => {
                // noop
            }
        }
    }
}

fn create_peer(id: u64, msg: String) -> (Service<DemoService>, JoinHandle<()>, JoinHandle<()>) {
    let peer_mgr = DemoPeerManager {
        addrs: Default::default(),
    };

    let (demo_proto, mut tx, rx) = TransmissionProtocol::build(DEMO_PROTOCOL_ID, peer_mgr);

    let key_pair = SecioKeyPair::secp256k1_generated();
    let pub_key = key_pair.to_public_key();

    // interval broadcast hello message to others
    let cast_handle = std::thread::spawn(move || {
        let interval_task = Interval::new(Instant::now(), Duration::from_secs(5))
            .for_each(move |_| {
                let hello_msg = message::Hello {
                    id,
                    msg: msg.clone(),
                };
                let _ = tx.try_send(CastMessage::All(hello_msg));

                Ok(())
            })
            .map_err(|err| error!("{}", err));

        tokio::run(interval_task);
    });

    // handle hello message from others
    let recv_handle = std::thread::spawn(move || {
        let recv_task = rx
            .for_each(|msg| {
                info!("Demo service: {:?}", msg);
                Ok(())
            })
            .map_err(|err| error!("{:?}", err));

        tokio::run(recv_task);
    });

    let service = ServiceBuilder::default()
        .insert_protocol(demo_proto)
        .forever(true)
        .key_pair(key_pair)
        .build(DemoService {
            _pub_key: Some(pub_key),
        });

    (service, cast_handle, recv_handle)
}

fn bootstrap_peer() {
    let (mut service, ..) = create_peer(1337, String::from("hello visitors"));
    let _ = service.listen("/ip4/127.0.0.1/tcp/1337".parse().unwrap());

    tokio::run(service.for_each(|_| Ok(())));
}

fn normal_peer(port: u64, hello_msg: String) {
    let (mut service, ..) = create_peer(port, hello_msg);
    service
        .dial(
            "/ip4/127.0.0.1/tcp/1337".parse().unwrap(),
            DialProtocol::All,
        )
        .unwrap();
    let _ = service
        .listen(format!("/ip4/127.0.0.1/tcp/{}", port).parse().unwrap())
        .unwrap();

    tokio::run(service.for_each(|_| Ok(())));
}

fn main() {
    env_logger::init();

    let arg = std::env::args().nth(1);
    if arg == Some("bootstrap".to_string()) {
        info!("Starting bootstrap peer ......");
        bootstrap_peer();
    } else {
        let port = arg.unwrap().parse::<u64>().unwrap();
        info!("Starting demo peer ......");
        normal_peer(port, format!("hello from {}", port));
    }
}
