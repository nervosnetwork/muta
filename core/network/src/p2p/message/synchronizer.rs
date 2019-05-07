use prost::Message as ProstMessage;
use uuid::Uuid;

use core_consensus::Status;
use core_serialization::Block as SerBlock;

#[derive(Clone, PartialEq, ProstMessage)]
pub struct BroadcastStatus {
    #[prost(bytes, tag = "1")]
    pub hash: Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub height: u64,
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PullBlocks {
    #[prost(string, tag = "1")]
    pub uuid: String,
    #[prost(uint64, repeated, tag = "2")]
    pub heights: Vec<u64>,
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PushBlocks {
    #[prost(string, tag = "1")]
    pub uuid: String,
    #[prost(message, repeated, tag = "2")]
    pub blocks: Vec<SerBlock>,
}

pub mod packed_message {
    use prost::Oneof;

    use super::{BroadcastStatus, PullBlocks, PushBlocks};

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        BroadcastStatus(BroadcastStatus),

        #[prost(message, tag = "2")]
        PushBlocks(PushBlocks),

        #[prost(message, tag = "3")]
        PullBlocks(PullBlocks),
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct SynchronizerMessage {
    #[prost(oneof = "packed_message::Message", tags = "1, 2, 3")]
    pub message: Option<packed_message::Message>,
}

impl SynchronizerMessage {
    pub fn broadcast_status(status: Status) -> Self {
        SynchronizerMessage {
            message: Some(packed_message::Message::BroadcastStatus(BroadcastStatus {
                hash:   status.hash.as_bytes().to_vec(),
                height: status.height,
            })),
        }
    }

    pub fn pull_blocks(uuid: Uuid, heights: Vec<u64>) -> Self {
        SynchronizerMessage {
            message: Some(packed_message::Message::PullBlocks(PullBlocks {
                uuid: uuid.to_string(),
                heights,
            })),
        }
    }

    pub fn push_blocks(uuid: String, blocks: Vec<SerBlock>) -> Self {
        SynchronizerMessage {
            message: Some(packed_message::Message::PushBlocks(PushBlocks {
                uuid,
                blocks,
            })),
        }
    }
}
