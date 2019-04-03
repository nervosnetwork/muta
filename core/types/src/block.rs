use std::convert::{From, Into};

use rlp::{Encodable, RlpStream};

use core_serialization::block::{Block as PbBlock, BlockHeader as PbBlockHeader};

use crate::{Address, Bloom, Hash};

#[derive(Default, Debug, Clone)]
pub struct BlockHeader {
    pub prevhash: Hash,
    pub timestamp: u64,
    pub height: u64,
    pub transactions_root: Hash,
    pub state_root: Hash,
    pub receipts_root: Hash,
    pub logs_bloom: Bloom,
    pub quota_used: u64,
    pub quota_limit: u64,
    pub votes: Vec<Hash>,
    pub proposer: Address,
}

impl BlockHeader {
    /// Calculate the block header hash. To maintain consistency we use RLP serialization.
    pub fn hash(&self) -> Hash {
        let rlp_data = rlp::encode(self);
        Hash::digest(&rlp_data)
    }
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
        s.append(&self.logs_bloom.as_ref());
        s.append(&self.quota_used);
        s.append(&self.quota_limit);
        s.append_list(&self.votes);
        s.append(&self.proposer);
    }
}

impl From<PbBlockHeader> for BlockHeader {
    fn from(header: PbBlockHeader) -> Self {
        BlockHeader {
            prevhash: Hash::from_bytes(&header.prevhash).expect("never returns an error"),
            timestamp: header.timestamp,
            height: header.height,
            transactions_root: Hash::from_bytes(&header.transactions_root)
                .expect("never returns an error"),
            state_root: Hash::from_bytes(&header.state_root).expect("never returns an error"),
            receipts_root: Hash::from_bytes(&header.receipts_root).expect("never returns an error"),
            logs_bloom: Bloom::from_slice(&header.logs_bloom),
            quota_used: header.quota_used,
            quota_limit: header.quota_limit,
            votes: header
                .votes
                .iter()
                .map(|v| Hash::from_bytes(v).expect("never returns an error"))
                .collect(),
            proposer: Address::from_bytes(&header.proposer).expect("never returns an error"),
        }
    }
}

impl Into<PbBlockHeader> for BlockHeader {
    fn into(self) -> PbBlockHeader {
        PbBlockHeader {
            prevhash: self.prevhash.as_bytes().to_vec(),
            timestamp: self.timestamp,
            height: self.height,
            transactions_root: self.transactions_root.as_bytes().to_vec(),
            state_root: self.state_root.as_bytes().to_vec(),
            receipts_root: self.receipts_root.as_bytes().to_vec(),
            logs_bloom: self.logs_bloom.as_bytes().to_vec(),
            quota_used: self.quota_used,
            quota_limit: self.quota_limit,
            votes: self.votes.iter().map(|v| v.as_bytes().to_vec()).collect(),
            proposer: self.proposer.as_bytes().to_vec(),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub tx_hashes: Vec<Hash>,
}

impl From<PbBlock> for Block {
    fn from(block: PbBlock) -> Self {
        let header = match block.header {
            Some(header) => BlockHeader::from(header),
            None => BlockHeader::default(),
        };

        Block {
            header,
            tx_hashes: block
                .tx_hashes
                .iter()
                .map(|tx_hash| Hash::from_bytes(&tx_hash).expect("never returns an error"))
                .collect(),
        }
    }
}

impl Into<PbBlock> for Block {
    fn into(self) -> PbBlock {
        PbBlock {
            header: Some(self.header.into()),
            tx_hashes: self
                .tx_hashes
                .iter()
                .map(|h| h.as_bytes().to_vec())
                .collect(),
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
