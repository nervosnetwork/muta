mod behaviour;
mod common;
mod identification;
mod message;
mod protocol;

use self::protocol::IdentifyProtocol;
use behaviour::IdentifyBehaviour;

use futures::channel::mpsc::UnboundedSender;
use tentacle::builder::MetaBuilder;
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::ProtocolId;

use crate::event::PeerManagerEvent;
use crate::peer_manager::PeerManagerHandle;

pub const NAME: &str = "chain_identify";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

#[derive(Clone)]
pub struct Identify {
    pub proto: IdentifyProtocol,
}

impl Identify {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        #[cfg(feature = "global_ip_only")]
        log::info!("turn on global ip only");
        #[cfg(not(feature = "global_ip_only"))]
        log::info!("turn off global ip only");

        let behaviour = IdentifyBehaviour::new(peer_mgr, event_tx);
        Identify {
            proto: IdentifyProtocol::new(behaviour),
        }
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || ProtocolHandle::Callback(Box::new(self.proto)))
            .build()
    }
}
