use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;
use tentacle::service::{ProtocolMeta, TargetProtocol};
use tentacle::ProtocolId;

use crate::event::PeerManagerEvent;
use crate::message::RawSessionMessage;
use crate::peer_manager::PeerManagerHandle;
use crate::protocols::discovery::Discovery;
use crate::protocols::identify::Identify;
use crate::protocols::ping::Ping;
use crate::protocols::transmitter::Transmitter;
use crate::traits::NetworkProtocol;

pub const PING_PROTOCOL_ID: usize = 1;
pub const IDENTIFY_PROTOCOL_ID: usize = 2;
pub const DISCOVERY_PROTOCOL_ID: usize = 3;
pub const TRANSMITTER_PROTOCOL_ID: usize = 4;

#[derive(Default)]
pub struct CoreProtocolBuilder {
    ping:        Option<Ping>,
    identify:    Option<Identify>,
    discovery:   Option<Discovery>,
    transmitter: Option<Transmitter>,
}

pub struct CoreProtocol {
    metas:    Vec<ProtocolMeta>,
    identify: Identify,
}

impl CoreProtocol {
    pub fn build() -> CoreProtocolBuilder {
        CoreProtocolBuilder::new()
    }

    pub fn identify(&self) -> Identify {
        self.identify.clone()
    }
}

impl NetworkProtocol for CoreProtocol {
    fn target() -> TargetProtocol {
        TargetProtocol::Single(ProtocolId::new(IDENTIFY_PROTOCOL_ID))
    }

    fn metas(self) -> Vec<ProtocolMeta> {
        self.metas
    }

    fn message_proto_id() -> ProtocolId {
        ProtocolId::new(TRANSMITTER_PROTOCOL_ID)
    }
}

impl CoreProtocolBuilder {
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

    pub fn identify(
        mut self,
        peer_mgr: PeerManagerHandle,
        event_tx: UnboundedSender<PeerManagerEvent>,
    ) -> Self {
        let identify = Identify::new(peer_mgr, event_tx);

        self.identify = Some(identify);
        self
    }

    pub fn discovery(
        mut self,
        peer_mgr: PeerManagerHandle,
        event_tx: UnboundedSender<PeerManagerEvent>,
        sync_interval: Duration,
    ) -> Self {
        let discovery = Discovery::new(peer_mgr, event_tx, sync_interval);

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

        let ping = ping.expect("init: missing protocol ping");
        let identify = identify.expect("init: missing protocol identify");
        let discovery = discovery.expect("init: missing protocol discovery");
        let transmitter = transmitter.expect("init: missing protocol transmitter");

        metas.push(ping.build_meta(PING_PROTOCOL_ID.into()));
        metas.push(identify.clone().build_meta(IDENTIFY_PROTOCOL_ID.into()));
        metas.push(discovery.build_meta(DISCOVERY_PROTOCOL_ID.into()));
        metas.push(transmitter.build_meta(TRANSMITTER_PROTOCOL_ID.into()));

        CoreProtocol { metas, identify }
    }
}
