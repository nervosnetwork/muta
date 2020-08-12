mod behaviour;
mod message;
mod protocol;

use std::time::Duration;

use tentacle::builder::MetaBuilder;
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::ProtocolId;

use crate::compression::Snappy;
use crate::peer_manager::PeerManagerHandle;
use crate::reactor::MessageRouter;

use self::behaviour::TransmitterBehaviour;
use self::protocol::TransmitterProtocol;
pub use message::{ReceivedMessage, Recipient, TransmitterMessage};

pub const NAME: &str = "chain_transmitter";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.3"];
pub const DATA_SEQ_TIMEOUT: Duration = Duration::from_secs(60);
pub const MAX_CHUNK_SIZE: usize = 4 * 1000 * 1000; // 4MB

#[derive(Clone)]
pub struct Transmitter {
    router:               MessageRouter<Snappy>,
    pub(crate) behaviour: TransmitterBehaviour,
    peer_mgr:             PeerManagerHandle,
}

impl Transmitter {
    pub fn new(router: MessageRouter<Snappy>, peer_mgr: PeerManagerHandle) -> Self {
        let behaviour = TransmitterBehaviour::new();
        Transmitter {
            router,
            behaviour,
            peer_mgr,
        }
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .session_handle(move || {
                let proto = TransmitterProtocol::new(self.router.clone(), self.peer_mgr.clone());
                ProtocolHandle::Callback(Box::new(proto))
            })
            .build()
    }
}
