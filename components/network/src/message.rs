use packed_message::Message;

use prost::Message as ProstMessage;

pub mod packed_message {
    use prost::Oneof;

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Message {
        #[prost(bytes, tag = "1")]
        Consensus(Vec<u8>), // change Vec<u8> to SignedMessage from PR #74
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub(crate) struct PackedMessage {
    #[prost(oneof = "Message", tags = "1")]
    pub message: Option<Message>,
}
