use bytes::Bytes;
use derive_more::Display;
use muta_codec_derive::RlpFixedCodec;
use serde::{Deserialize, Serialize};

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::{Address, Hash, MerkleRoot};
use crate::ProtocolResult;

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub header:            BlockHeader,
    pub ordered_tx_hashes: Vec<Hash>,
}

#[derive(RlpFixedCodec, Clone, Debug, Display, PartialEq, Eq)]
#[display(
    fmt = "chain id {:?}, height {}, exec height {}, previous hash {:?},
    ordered root {:?}, order_signed_transactions_hash {:?}, confirm root {:?}, state root {:?},
    receipt root {:?},cycles_used {:?}, proposer {:?}, proof {:?}, validators {:?}",
    chain_id,
    height,
    exec_height,
    prev_hash,
    order_root,
    order_signed_transactions_hash,
    confirm_root,
    state_root,
    receipt_root,
    cycles_used,
    proposer,
    proof,
    validators
)]
pub struct BlockHeader {
    pub chain_id:                       Hash,
    pub height:                         u64,
    pub exec_height:                    u64,
    pub prev_hash:                      Hash,
    pub timestamp:                      u64,
    pub order_root:                     MerkleRoot,
    pub order_signed_transactions_hash: Hash,
    pub confirm_root:                   Vec<MerkleRoot>,
    pub state_root:                     MerkleRoot,
    pub receipt_root:                   Vec<MerkleRoot>,
    pub cycles_used:                    Vec<u64>,
    pub proposer:                       Address,
    pub proof:                          Proof,
    pub validator_version:              u64,
    pub validators:                     Vec<Validator>,
}

#[derive(RlpFixedCodec, Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct Proof {
    pub height:     u64,
    pub round:      u64,
    pub block_hash: Hash,
    pub signature:  Bytes,
    pub bitmap:     Bytes,
}

#[derive(RlpFixedCodec, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Validator {
    pub pub_key:        Bytes,
    pub propose_weight: u32,
    pub vote_weight:    u32,
}

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct Pill {
    pub block:          Block,
    pub propose_hashes: Vec<Hash>,
}
