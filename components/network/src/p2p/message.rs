pub use packed_message::Message;

use prost::Message as ProstMessage;

#[derive(Clone, PartialEq, ProstMessage)]
pub(crate) struct PackedMessage {
    #[prost(oneof = "Message", tags = "1, 2, 3")]
    pub message: Option<Message>,
}

pub mod packed_message {
    use core_serialization::{Block, SignedTransaction};

    use prost::Oneof;

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Message {
        #[prost(bytes, tag = "1")]
        Consensus(Vec<u8>), // change Vec<u8> to SignedMessage from PR #74

        #[prost(message, tag = "2")]
        SignedTransaction(SignedTransaction),

        #[prost(message, tag = "3")]
        Block(Block),
    }
}

// Conversion from core-types to core-serialization
impl From<crate::Message> for Message {
    fn from(msg: crate::Message) -> Message {
        use crate::Message as CTMessage;

        match msg {
            CTMessage::Consensus(v) => Message::Consensus(v),
            CTMessage::SignedTransaction(stx) => Message::SignedTransaction((*stx).into()),
            CTMessage::Block(block) => Message::Block((*block).into()),
        }
    }
}
