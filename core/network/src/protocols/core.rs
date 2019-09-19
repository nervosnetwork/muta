use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;
use tentacle::{
    service::{DialProtocol, ProtocolMeta},
    ProtocolId,
};
use tentacle_discovery::AddressManager;

use crate::{
    event::PeerManagerEvent,
    message::RawSessionMessage,
    protocols::{discovery::Discovery, ping::Ping, transmitter::Transmitter},
    traits::NetworkProtocol,
};

pub const PING_PROTOCOL_ID: usize = 1;
pub const DISCOVERY_PROTOCOL_ID: usize = 2;
pub const TRANSMITTER_PROTOCOL_ID: usize = 3;

#[derive(Default)]
pub struct CoreProtocolBuilder<M> {
    ping:        Option<Ping>,
    discovery:   Option<Discovery<M>>,
    transmitter: Option<Transmitter>,
}

pub struct CoreProtocol {
    metas: Vec<ProtocolMeta>,
}

impl CoreProtocol {
    pub fn build<M: AddressManager + Send + 'static>() -> CoreProtocolBuilder<M> {
        CoreProtocolBuilder::new()
    }
}

impl NetworkProtocol for CoreProtocol {
    fn target() -> DialProtocol {
        DialProtocol::Multi(vec![
            ProtocolId::new(PING_PROTOCOL_ID),
            ProtocolId::new(DISCOVERY_PROTOCOL_ID),
            ProtocolId::new(TRANSMITTER_PROTOCOL_ID),
        ])
    }

    fn metas(self) -> Vec<ProtocolMeta> {
        self.metas
    }

    fn message_proto_id() -> ProtocolId {
        ProtocolId::new(TRANSMITTER_PROTOCOL_ID)
    }
}

impl<M: AddressManager + Send + 'static> CoreProtocolBuilder<M> {
    pub fn new() -> Self {
        CoreProtocolBuilder {
            ping:        None,
            discovery:   None,
            transmitter: None,
        }
    }

    pub fn ping(
        mut self,
        interval: Duration,
        timeout: Duration,
        event_tx: UnboundedSender<PeerManagerEvent>,
    ) -> Self {
        let ping = Ping::new(interval, timeout, event_tx);

        self.ping = Some(ping);
        self
    }

    pub fn discovery(mut self, addr_mgr: M, sync_interval: Duration) -> Self {
        let discovery = Discovery::new(addr_mgr, sync_interval);

        self.discovery = Some(discovery);
        self
    }

    pub fn transmitter(mut self, bytes_tx: UnboundedSender<RawSessionMessage>) -> Self {
        let transmitter = Transmitter::new(bytes_tx);

        self.transmitter = Some(transmitter);
        self
    }

    pub fn build(self) -> CoreProtocol {
        let mut metas = Vec::with_capacity(3);

        let CoreProtocolBuilder {
            ping,
            discovery,
            transmitter,
        } = self;

        // Panic early during protocol setup not runtime
        assert!(ping.is_some(), "init: missing protocol ping");
        assert!(discovery.is_some(), "init: missing protocol discovery");
        assert!(transmitter.is_some(), "init: missing protocol transmitter");

        if let Some(ping) = ping {
            metas.push(ping.build_meta(PING_PROTOCOL_ID.into()));
        }

        if let Some(discovery) = discovery {
            metas.push(discovery.build_meta(DISCOVERY_PROTOCOL_ID.into()));
        }

        if let Some(transmitter) = transmitter {
            metas.push(transmitter.build_meta(TRANSMITTER_PROTOCOL_ID.into()));
        }

        CoreProtocol { metas }
    }
}
