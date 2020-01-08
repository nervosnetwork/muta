use bytes::Bytes;

use crate::types::{Address, Bloom, Hash, MerkleRoot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Epoch {
    pub header:            EpochHeader,
    pub ordered_tx_hashes: Vec<Hash>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpochHeader {
    pub chain_id:          Hash,
    pub epoch_id:          u64,
    pub exec_epoch_id:     u64,
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

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Proof {
    pub epoch_id:   u64,
    pub round:      u64,
    pub epoch_hash: Hash,
    pub signature:  Bytes,
    pub bitmap:     Bytes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validator {
    pub address:        Address,
    pub propose_weight: u8,
    pub vote_weight:    u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pill {
    pub epoch:          Epoch,
    pub propose_hashes: Vec<Hash>,
}
