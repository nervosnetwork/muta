mod behaviour;
mod common;
mod message;
#[allow(dead_code)]
#[allow(clippy::all)]
mod message_mol; // Auto generated code
mod protocol;
use self::protocol::IdentifyProtocol;
use behaviour::IdentifyBehaviour;

use crate::{event::PeerManagerEvent, peer_manager::PeerManagerHandle};

use futures::channel::mpsc::UnboundedSender;
use tentacle::{
    builder::MetaBuilder,
    service::{ProtocolHandle, ProtocolMeta},
    ProtocolId,
};

pub const NAME: &str = "chain_identify";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub struct Identify(IdentifyProtocol);

impl Identify {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        #[cfg(feature = "global_ip_only")]
        log::info!("turn on global ip only");
        #[cfg(not(feature = "global_ip_only"))]
        log::info!("turn off global ip only");

        let behaviour = IdentifyBehaviour::new(peer_mgr, event_tx);
        Identify(IdentifyProtocol::new(behaviour))
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
