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

#[derive(Default, Debug, Clone)]
pub struct BlockBody {
    pub transaction_hashes: Vec<Hash>,
}

#[derive(Default, Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub body: BlockBody,
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
