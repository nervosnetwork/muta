use futures::channel::mpsc::UnboundedSender;
use log::error;
use tentacle::{
    builder::MetaBuilder,
    bytes::Bytes,
    context::{ProtocolContext, ProtocolContextMutRef},
    service::{ProtocolHandle, ProtocolMeta},
    traits::ServiceProtocol,
    ProtocolId,
};

use crate::message::RawSessionMessage;

pub const NAME: &str = "chain_transmitter";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub struct Transmitter {
    msg_deliver: UnboundedSender<RawSessionMessage>,
}

impl Transmitter {
    pub fn new(msg_deliver: UnboundedSender<RawSessionMessage>) -> Self {
        Transmitter { msg_deliver }
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || ProtocolHandle::Callback(Box::new(self)))
            .build()
    }
}

impl ServiceProtocol for Transmitter {
    fn init(&mut self, _ctx: &mut ProtocolContext) {
        // Nothing to init
    }

    fn received(&mut self, ctx: ProtocolContextMutRef, data: Bytes) {
        let raw_msg = RawSessionMessage::new(ctx.session.id, data);

        if self.msg_deliver.unbounded_send(raw_msg).is_err() {
            error!("network: transmitter: msg receiver dropped");
        }
    }
}
