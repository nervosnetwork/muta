use bytes::Bytes;

use crate::types::{AccountAddress, Bloom, Hash, MerkleRoot};

#[derive(Clone, Debug)]
pub struct Epoch {
    pub header:            EpochHeader,
    pub ordered_tx_hashes: Vec<Hash>,
}

#[derive(Clone, Debug)]
pub struct EpochHeader {
    pub chain_id:          Hash,
    pub epoch_id:          u64,
    pub pre_hash:          Hash,
    pub timestamp:         u64,
    pub logs_bloom:        Bloom,
    pub order_root:        MerkleRoot,
    pub confirm_root:      Vec<MerkleRoot>,
    pub state_root:        MerkleRoot,
    pub receipt_root:      Vec<MerkleRoot>,
    pub cycles_used:       u64,
    pub proposer:          AccountAddress,
    pub proof:             Proof,
    pub validator_version: u64,
    pub validators:        Vec<Validator>,
}

#[derive(Clone, Debug)]
pub struct Proof {
    pub epoch_id:   u64,
    pub round:      u64,
    pub epoch_hash: Hash,
    pub signature:  Bytes,
}

#[derive(Clone, Debug)]
pub struct Validator {
    pub address: AccountAddress,
    pub weight:  u64,
}

#[derive(Clone, Debug)]
pub struct Pill {
    pub epoch:          Epoch,
    pub propose_hashes: Vec<Hash>,
}

#[derive(Clone, Debug)]
pub struct EpochId {
    pub id: u64,
}
