use futures::channel::mpsc::UnboundedSender;
use log::error;
use protocol::BytesMut;
use tentacle::{
    builder::MetaBuilder,
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

    fn received(&mut self, ctx: ProtocolContextMutRef, data: tentacle::bytes::Bytes) {
        let pubkey = ctx.session.remote_pubkey.as_ref();
        let peer_id = pubkey.expect("impossible, no public key").peer_id();
        let data = BytesMut::from(data.as_ref()).freeze();

        let raw_msg = RawSessionMessage::new(ctx.session.id, peer_id, data);
        if self.msg_deliver.unbounded_send(raw_msg).is_err() {
            error!("network: transmitter: msg receiver dropped");
        }
    }
}
