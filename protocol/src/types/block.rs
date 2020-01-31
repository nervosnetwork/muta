use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::types::{Address, Bloom, Hash, MerkleRoot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub header:            BlockHeader,
    pub ordered_tx_hashes: Vec<Hash>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockHeader {
    pub chain_id:          Hash,
    pub height:            u64,
    pub exec_height:       u64,
    pub pre_hash:          Hash,
    pub timestamp:         u64,
    pub logs_bloom:        Vec<Bloom>,
    pub order_root:        MerkleRoot,
    pub confirm_root:      Vec<MerkleRoot>,
    pub state_root:        MerkleRoot,
    pub receipt_root:      Vec<MerkleRoot>,
    pub cycles_used:       Vec<u64>,
    pub proposer:          Address,
    pub proof:             Proof,
    pub validator_version: u64,
    pub validators:        Vec<Validator>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct Proof {
    pub height:     u64,
    pub round:      u64,
    pub block_hash: Hash,
    pub signature:  Bytes,
    pub bitmap:     Bytes,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Validator {
    pub address:        Address,
    pub propose_weight: u8,
    pub vote_weight:    u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pill {
    pub block:          Block,
    pub propose_hashes: Vec<Hash>,
}
