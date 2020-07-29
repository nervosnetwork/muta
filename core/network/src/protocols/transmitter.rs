mod behaviour;
mod message;
mod protocol;

use futures::channel::mpsc::UnboundedSender;
use tentacle::builder::MetaBuilder;
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::ProtocolId;

use self::behaviour::TransmitterBehaviour;
use self::protocol::TransmitterProtocol;
pub use message::{ReceivedMessage, Recipient, TransmitterMessage};

pub const NAME: &str = "chain_transmitter";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.3"];

#[derive(Clone)]
pub struct Transmitter {
    data_tx:              UnboundedSender<ReceivedMessage>,
    pub(crate) behaviour: TransmitterBehaviour,
}

impl Transmitter {
    pub fn new(data_tx: UnboundedSender<ReceivedMessage>) -> Self {
        let behaviour = TransmitterBehaviour::new();
        Transmitter { data_tx, behaviour }
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .session_handle(move || {
                let proto = TransmitterProtocol::new(self.data_tx.clone());
                ProtocolHandle::Callback(Box::new(proto))
            })
            .build()
    }
}
