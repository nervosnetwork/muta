use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;
use tentacle::{
    service::{DialProtocol, ProtocolMeta},
    ProtocolId,
};
use tentacle_discovery::AddressManager;
use tentacle_identify::Callback;

use crate::{
    event::PeerManagerEvent,
    message::RawSessionMessage,
    protocols::{discovery::Discovery, identify::Identify, ping::Ping, transmitter::Transmitter},
    traits::NetworkProtocol,
};

pub const PING_PROTOCOL_ID: usize = 1;
pub const IDENTIFY_PROTOCOL_ID: usize = 2;
pub const DISCOVERY_PROTOCOL_ID: usize = 3;
pub const TRANSMITTER_PROTOCOL_ID: usize = 4;

#[derive(Default)]
pub struct CoreProtocolBuilder<M, C> {
    ping:        Option<Ping>,
    identify:    Option<Identify<C>>,
    discovery:   Option<Discovery<M>>,
    transmitter: Option<Transmitter>,
}

pub struct CoreProtocol {
    metas: Vec<ProtocolMeta>,
}

impl CoreProtocol {
    pub fn build<M, C>() -> CoreProtocolBuilder<M, C>
    where
        M: AddressManager + Send + 'static,
        C: Callback + Send + 'static,
    {
        CoreProtocolBuilder::new()
    }
}

impl NetworkProtocol for CoreProtocol {
    fn target() -> DialProtocol {
        DialProtocol::Multi(vec![
            ProtocolId::new(PING_PROTOCOL_ID),
            ProtocolId::new(IDENTIFY_PROTOCOL_ID),
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

impl<M, C> CoreProtocolBuilder<M, C>
where
    M: AddressManager + Send + 'static,
    C: Callback + Send + 'static,
{
    pub fn new() -> Self {
        CoreProtocolBuilder {
            ping:        None,
            identify:    None,
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

    pub fn identify(mut self, callback: C) -> Self {
        let identify = Identify::new(callback);

        self.identify = Some(identify);
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
        let mut metas = Vec::with_capacity(4);

        let CoreProtocolBuilder {
            ping,
            identify,
            discovery,
            transmitter,
        } = self;

        // Panic early during protocol setup not runtime
        assert!(ping.is_some(), "init: missing protocol ping");
        assert!(identify.is_some(), "init: missing protocol identify");
        assert!(discovery.is_some(), "init: missing protocol discovery");
        assert!(transmitter.is_some(), "init: missing protocol transmitter");

        if let Some(ping) = ping {
            metas.push(ping.build_meta(PING_PROTOCOL_ID.into()));
        }

        if let Some(identify) = identify {
            metas.push(identify.build_meta(IDENTIFY_PROTOCOL_ID.into()));
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
