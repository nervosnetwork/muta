use std::convert::{From, Into};

use rlp::{Encodable, RlpStream};

use core_serialization::block::{Block as PbBlock, BlockHeader as PbBlockHeader};

use crate::{Address, Hash};

#[derive(Default, Debug, Clone)]
pub struct BlockHeader {
    pub prevhash: Hash,
    pub timestamp: u64,
    pub height: u64,
    pub transactions_root: Hash,
    pub state_root: Hash,
    pub receipts_root: Hash,
    pub quota_used: u64,
    pub quota_limit: u64,
    pub votes: Vec<Hash>,
    pub proposer: Address,
}

/// Structure encodable to RLP
impl Encodable for BlockHeader {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.prevhash);
        s.append(&self.timestamp);
        s.append(&self.height);
        s.append(&self.transactions_root);
        s.append(&self.state_root);
        s.append(&self.receipts_root);
        s.append(&self.quota_used);
        s.append(&self.quota_limit);
        s.append_list(&self.votes);
        s.append(&self.proposer);
    }
}

impl From<PbBlockHeader> for BlockHeader {
    fn from(header: PbBlockHeader) -> Self {
        BlockHeader {
            prevhash: Hash::from_raw(&header.prevhash),
            timestamp: header.timestamp,
            height: header.height,
            transactions_root: Hash::from_raw(&header.transactions_root),
            state_root: Hash::from_raw(&header.state_root),
            receipts_root: Hash::from_raw(&header.receipts_root),
            quota_used: header.quota_used,
            quota_limit: header.quota_limit,
            votes: header.votes.iter().map(|v| Hash::from_raw(v)).collect(),
            proposer: Address::from(header.proposer.as_ref()),
        }
    }
}

impl Into<PbBlockHeader> for BlockHeader {
    fn into(self) -> PbBlockHeader {
        PbBlockHeader {
            prevhash: self.prevhash.as_ref().to_vec(),
            timestamp: self.timestamp,
            height: self.height,
            transactions_root: self.transactions_root.as_ref().to_vec(),
            state_root: self.state_root.as_ref().to_vec(),
            receipts_root: self.receipts_root.as_ref().to_vec(),
            quota_used: self.quota_used,
            quota_limit: self.quota_limit,
            votes: self.votes.iter().map(|v| v.as_ref().to_vec()).collect(),
            proposer: self.proposer.as_ref().to_vec(),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub tx_hashes: Vec<Hash>,
}

impl Block {
    /// Calculate the block hash. To maintain consistency we use RLP serialization.
    pub fn hash(&self) -> Hash {
        let rlp_data = rlp::encode(self);
        Hash::from_raw(&rlp_data)
    }
}

/// Structure encodable to RLP
impl Encodable for Block {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.header);
        s.append_list(&self.tx_hashes);
    }
}

impl From<PbBlock> for Block {
    fn from(block: PbBlock) -> Self {
        Block {
            header: BlockHeader::from(block.header.unwrap()),
            tx_hashes: block.tx_hashes.iter().map(|h| Hash::from_raw(h)).collect(),
        }
    }
}

impl Into<PbBlock> for Block {
    fn into(self) -> PbBlock {
        PbBlock {
            header: Some(self.header.into()),
            tx_hashes: self.tx_hashes.iter().map(|h| h.as_ref().to_vec()).collect(),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Proposal {
    pub block: Block,
    pub lock_round: u64,
    pub lock_votes: Vec<Vote>,
    pub round: u64,
    pub height: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

#[derive(Debug, Clone)]
pub struct Vote {
    pub vote_type: VoteType,
    pub height: u64,
    pub round: u64,
    pub address: Address,
    pub hash: Hash,
}
