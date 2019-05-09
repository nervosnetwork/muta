use prost::Message as ProstMessage;
use uuid::Uuid;

use core_consensus::Status;
use core_serialization::{Block as SerBlock, SignedTransaction as SerSignedTransaction};
use core_types::Hash;

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

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PullTxsSync {
    #[prost(string, tag = "1")]
    pub uuid: String,
    #[prost(bytes, repeated, tag = "2")]
    pub hashes: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PushTxsSync {
    #[prost(string, tag = "1")]
    pub uuid: String,
    #[prost(message, repeated, tag = "2")]
    pub sig_txs: Vec<SerSignedTransaction>,
}

pub mod packed_message {
    use prost::Oneof;

    use super::{BroadcastStatus, PullBlocks, PullTxsSync, PushBlocks, PushTxsSync};

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        BroadcastStatus(BroadcastStatus),

        #[prost(message, tag = "2")]
        PushBlocks(PushBlocks),

        #[prost(message, tag = "3")]
        PullBlocks(PullBlocks),

        #[prost(message, tag = "4")]
        PullTxsSync(PullTxsSync),

        #[prost(message, tag = "5")]
        PushTxsSync(PushTxsSync),
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct SynchronizerMessage {
    #[prost(oneof = "packed_message::Message", tags = "1, 2, 3, 4, 5")]
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

    pub fn pull_txs_sync(uuid: Uuid, hashes: Vec<Hash>) -> Self {
        let hashes = hashes
            .into_iter()
            .map(|h| h.as_bytes().to_vec())
            .collect::<_>();
        SynchronizerMessage {
            message: Some(packed_message::Message::PullTxsSync(PullTxsSync {
                uuid: uuid.to_string(),
                hashes,
            })),
        }
    }

    pub fn push_txs_sync(uuid: String, sig_txs: Vec<SerSignedTransaction>) -> Self {
        SynchronizerMessage {
            message: Some(packed_message::Message::PushTxsSync(PushTxsSync {
                uuid,
                sig_txs,
            })),
        }
    }
}
