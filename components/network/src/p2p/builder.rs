use crate::message::PackedMessage;
use crate::p2p::{Config, Service, ServiceWorker};

use core_p2p::{transmission::CastMessage, DefaultPeerManager, TransmissionProtocol};
use core_p2p::{ConnecProtocol, DiscoveryProtocol, IdentifyProtocol, PingProtocol};

use futures::sync::mpsc::{Receiver, Sender};
use tentacle::builder::ServiceBuilder;
use tentacle::ProtocolId;

use std::default::Default;

const INIT_PROTOCOL_ID: ProtocolId = 1;

pub struct Builder {
    service_builder: ServiceBuilder,
    peer_manager: DefaultPeerManager, // FIXME: make `PeerManager` trait object

    msg_tx: Sender<CastMessage<PackedMessage>>,
    msg_rx: Receiver<PackedMessage>,

    config: Config,
}

impl Builder {
    pub fn launch(self) -> Service {
        let config = self.config;

        let mut peer_manager = self.peer_manager;
        peer_manager.register_self(config.peer_id(), vec![config.listening_address()]);

        // kick start p2p service
        let builder = self.service_builder;
        let service_worker = ServiceWorker::kick_start(Service::service_worker(builder, &config));

        // kick start transmission msg rx handling
        let msg_rx = self.msg_rx;
        let transmit_worker = ServiceWorker::kick_start(Service::transmit_worker(msg_rx));

        Service {
            peer_manager,
            config,

            msg_tx: self.msg_tx,

            transmit_worker,
            service_worker,
        }
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }
}

impl Default for Builder {
    fn default() -> Self {
        let peer_manager = DefaultPeerManager::new();
        let config = Config::default();

        let ident = IdentifyProtocol::build(INIT_PROTOCOL_ID, peer_manager.clone());
        let disc = DiscoveryProtocol::build(INIT_PROTOCOL_ID + 1, peer_manager.clone());
        let connec = ConnecProtocol::build(INIT_PROTOCOL_ID + 2, peer_manager.clone());
        let ping = PingProtocol::build(INIT_PROTOCOL_ID + 3, peer_manager.clone());
        let (transmit, msg_tx, msg_rx) =
            TransmissionProtocol::build(INIT_PROTOCOL_ID + 4, peer_manager.clone());

        let service_builder = ServiceBuilder::default()
            .insert_protocol(ident)
            .insert_protocol(disc)
            .insert_protocol(connec)
            .insert_protocol(ping)
            .insert_protocol(transmit)
            .forever(true);

        Builder {
            service_builder,
            peer_manager,

            msg_tx,
            msg_rx,

            config,
        }
    }
}
