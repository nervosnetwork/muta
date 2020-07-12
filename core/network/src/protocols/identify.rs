mod behaviour;
mod message;
#[allow(dead_code)]
mod message_mol;
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
        let mut behaviour = IdentifyBehaviour::new(peer_mgr, event_tx);

        #[cfg(not(feature = "global_ip_only"))]
        {
            log::info!("network: turn off global ip only");
            behaviour.set_global_ip_only(false);
        }
        #[cfg(feature = "global_ip_only")]
        {
            log::info!("network: turn on global ip only");
            behaviour.set_global_ip_only(true);
        }

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
