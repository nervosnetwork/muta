mod behaviour;
mod message;
mod protocol;
use self::protocol::PingProtocol;
use behaviour::{EventTranslator, PingEventReporter};

use crate::event::PeerManagerEvent;

use futures::channel::mpsc::{self, UnboundedSender};
use tentacle::{
    builder::MetaBuilder,
    service::{ProtocolHandle, ProtocolMeta},
    ProtocolId,
};

use std::time::Duration;

pub const NAME: &str = "chain_ping";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub struct Ping(PingProtocol);

impl Ping {
    pub fn new(
        interval: Duration,
        timeout: Duration,
        sender: UnboundedSender<PeerManagerEvent>,
    ) -> Self {
        let reporter = PingEventReporter::new(sender);
        let (tx, rx) = mpsc::channel(1000);
        let translator = EventTranslator::new(rx, reporter);
        tokio::spawn(translator);

        Ping(PingProtocol::new(interval, timeout, tx))
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || ProtocolHandle::Callback(Box::new(self.0)))
            .build()
    }
}
