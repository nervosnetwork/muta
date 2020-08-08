mod behaviour;
mod common;
mod identification;
mod message;
mod protocol;

use std::sync::Arc;

use futures::channel::mpsc::UnboundedSender;
use tentacle::builder::MetaBuilder;
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::{ProtocolId, SessionId};

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

    pub fn wait_identified(session_id: SessionId) -> WaitIdentification {
        IdentifyProtocol::wait(session_id)
    }
}
