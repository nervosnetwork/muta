use std::default::Default;

use futures::sync::mpsc::{Receiver, Sender};
use tentacle::builder::ServiceBuilder;
use tentacle::ProtocolId;

use core_p2p::{
    transmission::{CastMessage, RecvMessage},
    DefaultPeerManager, TransmissionProtocol,
};
use core_p2p::{ConnecProtocol, DiscoveryProtocol, IdentifyProtocol, PingProtocol};

use crate::p2p::{Broadcaster, Config, PackedMessage, Service, ServiceWorker};
use crate::reactor::{OutboundMessage, Reaction, Reactor, ReactorMessage};

const INIT_PROTOCOL_ID: usize = 1;

struct PartialBuilder {
    service_builder: ServiceBuilder,
    peer_manager:    DefaultPeerManager, // FIXME: make `PeerManager` trait object

    msg_tx: Sender<CastMessage<PackedMessage>>,
    msg_rx: Receiver<RecvMessage<PackedMessage>>,

    config: Config,
}

impl Default for PartialBuilder {
    fn default() -> Self {
        let peer_manager = DefaultPeerManager::new();
        let config = Config::default();

        let ident =
            IdentifyProtocol::build(ProtocolId::new(INIT_PROTOCOL_ID), peer_manager.clone());
        let disc =
            DiscoveryProtocol::build(ProtocolId::new(INIT_PROTOCOL_ID + 1), peer_manager.clone());
        let connec =
            ConnecProtocol::build(ProtocolId::new(INIT_PROTOCOL_ID + 2), peer_manager.clone());
        let ping = PingProtocol::build(ProtocolId::new(INIT_PROTOCOL_ID + 3), peer_manager.clone());
        let (transmit, msg_tx, msg_rx) = TransmissionProtocol::build(
            ProtocolId::new(INIT_PROTOCOL_ID + 4),
            peer_manager.clone(),
        );

        let service_builder = ServiceBuilder::default()
            .insert_protocol(ident)
            .insert_protocol(disc)
            .insert_protocol(connec)
            .insert_protocol(ping)
            .insert_protocol(transmit)
            .forever(true);

        PartialBuilder {
            service_builder,
            peer_manager,

            msg_tx,
            msg_rx,

            config,
        }
    }
}

pub struct Builder<R> {
    service_builder: ServiceBuilder,
    peer_manager:    DefaultPeerManager,

    msg_tx: Sender<CastMessage<PackedMessage>>,
    msg_rx: Receiver<RecvMessage<PackedMessage>>,

    reactor:     R,
    outbound_rx: Receiver<OutboundMessage>,

    config: Config,
}

impl<R> Builder<R>
where
    R: Reactor<Input = ReactorMessage, Output = Reaction<ReactorMessage>> + Send + 'static,
{
    pub fn new(reactor: R, outbound_rx: Receiver<OutboundMessage>) -> Self {
        let PartialBuilder {
            service_builder,
            peer_manager,
            msg_tx,
            msg_rx,
            config,
        } = PartialBuilder::default();

        Builder {
            service_builder,
            peer_manager,

            msg_tx,
            msg_rx,

            reactor,
            outbound_rx,

            config,
        }
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn launch(self) -> Service {
        let config = self.config;

        let mut peer_manager = self.peer_manager;
        peer_manager.register_self(vec![config.listening_address()]);

        // kick start p2p service
        let builder = self.service_builder;
        let service_worker = ServiceWorker::kick_start(Service::service_worker(
            builder,
            &config,
            peer_manager.clone(),
        ));

        // kick start transmission msg rx handling
        let msg_rx = self.msg_rx;
        let reactor = self.reactor;
        let outbound_rx = self.outbound_rx;
        let broadcaster = Broadcaster::new(self.msg_tx.clone());
        let transmit_worker = ServiceWorker::kick_start(Service::transmit_worker(
            msg_rx,
            outbound_rx,
            broadcaster,
            reactor,
        ));

        Service {
            peer_manager,
            config,

            msg_tx: self.msg_tx,

            transmit_worker,
            service_worker,
        }
    }
}
