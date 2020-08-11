use futures::channel::mpsc::UnboundedSender;
use log::error;
use tentacle::builder::MetaBuilder;
use tentacle::context::{ProtocolContext, ProtocolContextMutRef};
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::traits::ServiceProtocol;
use tentacle::ProtocolId;

use crate::message::RawSessionMessage;
use crate::peer_manager::PeerManagerHandle;

pub const NAME: &str = "chain_transmitter";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.2"];

pub struct Transmitter {
    msg_deliver: UnboundedSender<RawSessionMessage>,
    peer_mgr:    PeerManagerHandle,
}

impl Transmitter {
    pub fn new(
        msg_deliver: UnboundedSender<RawSessionMessage>,
        peer_mgr: PeerManagerHandle,
    ) -> Self {
        Transmitter {
            msg_deliver,
            peer_mgr,
        }
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

    fn connected(&mut self, context: ProtocolContextMutRef, _version: &str) {
        if !self.peer_mgr.contains_session(context.session.id) {
            let _ = context.close_protocol(context.session.id, context.proto_id());
            return;
        }

        let peer_id = match context.session.remote_pubkey.as_ref() {
            Some(pubkey) => pubkey.peer_id(),
            None => {
                log::warn!("peer connection must be encrypted");
                let _ = context.disconnect(context.session.id);
                return;
            }
        };
        crate::protocols::OpenedProtocols::register(peer_id, context.proto_id());
    }

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
