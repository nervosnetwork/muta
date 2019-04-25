#![feature(async_await, await_macro, futures_api)]

use core_p2p::transmission::{
    CastMessage, Misbehavior, MisbehaviorResult, PeerManager, TransmissionProtocol,
};

use env_logger;
use futures::prelude::{Async, Stream};
use futures03::compat::Stream01CompatExt;
use futures03::future::ready;
use futures03::prelude::StreamExt;
use log::{error, info};
use parking_lot::RwLock;
use runtime::task::{spawn, JoinHandle};
use tentacle::{
    builder::ServiceBuilder,
    context::ServiceContext,
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey, SecioKeyPair},
    service::{DialProtocol, Service, ServiceError, ServiceEvent},
    traits::ServiceHandle,
    ProtocolId,
};
use tokio::timer::Interval;

use std::clone::Clone;
use std::collections::HashMap;
use std::sync::Arc;
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
        pub id:  u64,
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
    fn misbehave(
        &mut self,
        _: Option<PeerId>,
        multiaddr: Multiaddr,
        _kind: Misbehavior,
    ) -> MisbehaviorResult {
        let mut addrs = self.addrs.write();
        let value = addrs.entry(multiaddr).or_insert(100);
        *value -= 20;

        if *value <= 0 {
            MisbehaviorResult::Disconnect
        } else {
            MisbehaviorResult::Continue
        }
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
    fn handle_error(&mut self, _: &mut ServiceContext, error: ServiceError) {
        error!("Demo service error: {:?}", error);
    }

    fn handle_event(&mut self, _: &mut ServiceContext, event: ServiceEvent) {
        info!("Demo service event: {:?}", event);

        if let ServiceEvent::SessionClose {
            session_context: session,
        } = event
        {
            info!("Demo service: session {} disconnected", session.id);
        }
    }
}

fn create_peer(id: u64, msg: String) -> (Service<DemoService>, JoinHandle<()>, JoinHandle<()>) {
    let peer_mgr = DemoPeerManager {
        addrs: Default::default(),
    };

    let (demo_proto, mut tx, mut rx) = TransmissionProtocol::build(DEMO_PROTOCOL_ID, peer_mgr);

    let key_pair = SecioKeyPair::secp256k1_generated();
    let pub_key = key_pair.to_public_key();

    // interval broadcast hello message to others
    let cast_handle = spawn(async move {
        let interval_task = Interval::new(Instant::now(), Duration::from_secs(2))
            .compat()
            .for_each(move |_| {
                let hello_msg = message::Hello {
                    id,
                    msg: msg.clone(),
                };
                let _ = tx.try_send(CastMessage::All(hello_msg));

                ready(())
            });

        await!(interval_task);
    });

    // handle hello message from others
    let recv_handle = spawn(async move {
        let recv_task = Interval::new(Instant::now(), Duration::from_secs(5))
            .compat()
            .for_each(move |_| {
                match rx.poll() {
                    Ok(Async::Ready(Some(msg))) => {
                        info!("Demo service: {:?}", msg);
                    }
                    Err(err) => error!("Demo service: {:?}", err),
                    _ => {
                        // no-op
                    }
                }

                ready(())
            });

        await!(recv_task);
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

async fn bootstrap_peer() {
    let (mut service, ..) = create_peer(1337, String::from("hello visitors"));
    let _ = service.listen("/ip4/127.0.0.1/tcp/1337".parse().unwrap());

    await!(service.compat().for_each(|_| ready(())));
}

async fn normal_peer(port: u64, hello_msg: String) {
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

    await!(service.compat().for_each(|_| ready(())));
}

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    env_logger::init();

    let arg = std::env::args().nth(1);
    if arg == Some("bootstrap".to_string()) {
        info!("Starting bootstrap peer ......");
        await!(bootstrap_peer());
    } else {
        let port = arg.unwrap().parse::<u64>().unwrap();
        info!("Starting demo peer ......");
        await!(normal_peer(port, format!("hello from {}", port)));
    }
}
