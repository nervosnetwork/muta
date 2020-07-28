use futures::channel::mpsc::UnboundedSender;
use log::error;
use tentacle::{
    builder::MetaBuilder,
    context::ProtocolContextMutRef,
    service::{ProtocolHandle, ProtocolMeta},
    traits::SessionProtocol,
    ProtocolId,
};

use crate::message::RawSessionMessage;

pub const NAME: &str = "chain_transmitter";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.2"];

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
            .session_handle(move || {
                let transmitter = Transmitter {
                    msg_deliver: self.msg_deliver.clone(),
                };
                ProtocolHandle::Callback(Box::new(transmitter))
            })
            .build()
    }
}

impl SessionProtocol for Transmitter {
    fn received(&mut self, ctx: ProtocolContextMutRef, data: tentacle::bytes::Bytes) {
        let pubkey = ctx.session.remote_pubkey.as_ref();
        // Peers without encryption will not able to connect to us.
        let peer_id = pubkey.expect("impossible, no public key").peer_id();

        let raw_msg = RawSessionMessage::new(ctx.session.id, peer_id, data);
        if self.msg_deliver.unbounded_send(raw_msg).is_err() {
            error!("network: transmitter: msg receiver dropped");
        }
    }
}
