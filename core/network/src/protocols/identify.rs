mod behaviour;
mod common;
mod identification;
mod message;
mod protocol;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use futures::channel::mpsc::UnboundedSender;
use tentacle::builder::MetaBuilder;
use tentacle::secio::PeerId;
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::ProtocolId;

use crate::event::PeerManagerEvent;
use crate::peer_manager::PeerManagerHandle;

use self::protocol::IdentifyProtocol;
use behaviour::IdentifyBehaviour;

pub use self::identification::WaitIdentification;
pub use self::protocol::Error;

pub const NAME: &str = "chain_identify";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub struct Identify {
    behaviour: Arc<IdentifyBehaviour>,
}

impl Identify {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        #[cfg(feature = "global_ip_only")]
        log::info!("turn on global ip only");
        #[cfg(not(feature = "global_ip_only"))]
        log::info!("turn off global ip only");

        let behaviour = Arc::new(IdentifyBehaviour::new(peer_mgr, event_tx));
        Identify { behaviour }
    }

    #[cfg(not(test))]
    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        let behaviour = self.behaviour;

        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .session_handle(move || {
                ProtocolHandle::Callback(Box::new(IdentifyProtocol::new(Arc::clone(&behaviour))))
            })
            .build()
    }

    #[cfg(test)]
    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .session_handle(move || ProtocolHandle::Callback(Box::new(IdentifyProtocol::new())))
            .build()
    }

    pub fn wait_identified(peer_id: PeerId) -> WaitIdentification {
        IdentifyProtocol::wait(peer_id)
    }

    pub fn wait_failed(peer_id: &PeerId, error: String) {
        IdentifyProtocol::wait_failed(peer_id, error)
    }
}
