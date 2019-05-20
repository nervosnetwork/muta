use std::convert::TryInto;

use prost::Message as ProstMessage;

use core_serialization::{Block as SerBlock, CodecError};
use core_types::{Block, Hash};

#[derive(Clone, PartialEq, ProstMessage)]
pub struct BroadcastStatus {
    #[prost(bytes, tag = "1")]
    pub hash: Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub height: u64,
}

impl BroadcastStatus {
    pub fn from(hash: Hash, height: u64) -> Self {
        let hash = hash.as_bytes().to_vec();

        BroadcastStatus { hash, height }
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PullBlocks {
    #[prost(uint64, tag = "1")]
    pub uid: u64,
    #[prost(uint64, repeated, tag = "2")]
    pub heights: Vec<u64>,
}

impl PullBlocks {
    pub fn from(uid: u64, heights: Vec<u64>) -> Self {
        PullBlocks { uid, heights }
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PushBlocks {
    #[prost(uint64, tag = "1")]
    pub uid: u64,
    #[prost(message, repeated, tag = "2")]
    pub blocks: Vec<SerBlock>,
}

impl PushBlocks {
    pub fn from(uid: u64, blocks: Vec<Block>) -> Self {
        let blocks = blocks.into_iter().map(Into::into).collect::<Vec<_>>();

        PushBlocks { uid, blocks }
    }

    pub fn des(self) -> Result<Vec<Block>, CodecError> {
        self.blocks
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, CodecError>>()
    }
}
