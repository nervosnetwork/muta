#![feature(test)]
#![allow(clippy::type_complexity, clippy::suspicious_else_formatting)]

pub mod adapter;
pub mod consensus;
mod engine;
pub mod fixed_types;
pub mod message;
pub mod status;
pub mod synchronization;
#[cfg(test)]
mod tests;
pub mod util;
pub mod wal;
mod wal_proto;

use std::error::Error;

use derive_more::Display;

use common_crypto::Error as CryptoError;

use protocol::types::Hash;
use protocol::{ProtocolError, ProtocolErrorKind};

pub use crate::adapter::OverlordConsensusAdapter;
pub use crate::consensus::OverlordConsensus;
pub use crate::synchronization::{OverlordSynchronization, RichBlock};
pub use crate::wal::SignedTxsWAL;
pub use overlord::{types::Node, DurationConfig};

#[derive(Clone, Debug, Display, PartialEq, Eq)]
pub enum ConsensusType {
    #[display(fmt = "Signed Proposal")]
    SignedProposal,

    #[display(fmt = "Signed Vote")]
    SignedVote,

    #[display(fmt = "Aggregated Vote")]
    AggregateVote,

    #[display(fmt = "Rich Height")]
    RichHeight,

    #[display(fmt = "Rpc Pull Blocks")]
    RpcPullBlocks,

    #[display(fmt = "Rpc Pull Transactions")]
    RpcPullTxs,

    #[display(fmt = "Signed Choke")]
    SignedChoke,

    #[display(fmt = "WAL Signed Transactions")]
    WALSignedTxs,
}

/// Consensus errors defines here.
#[derive(Debug, Display)]
pub enum ConsensusError {
    /// Send consensus message error.
    #[display(fmt = "Send {:?} message failed", _0)]
    SendMsgErr(ConsensusType),

    /// Check block error.
    #[display(fmt = "Check invalid prev_hash, expect {:?} get {:?}", expect, actual)]
    InvalidPrevhash { expect: Hash, actual: Hash },

    #[display(fmt = "Check invalid status vec")]
    InvalidStatusVec,

    /// Decode consensus message error.
    #[display(fmt = "Decode {:?} message failed", _0)]
    DecodeErr(ConsensusType),

    /// Encode consensus message error.
    #[display(fmt = "Encode {:?} message failed", _0)]
    EncodeErr(ConsensusType),

    /// Overlord consensus protocol error.
    #[display(fmt = "Overlord error {:?}", _0)]
    OverlordErr(Box<dyn Error + Send>),

    /// Consensus missed last block proof.
    #[display(fmt = "Consensus missed proof of {} block", _0)]
    MissingProof(u64),

    /// Consensus missed the pill.
    #[display(fmt = "Consensus missed pill cooresponding {:?}", _0)]
    MissingPill(Hash),

    /// Consensus missed the block header.
    #[display(fmt = "Consensus missed block header of {} block", _0)]
    MissingBlockHeader(u64),

    /// This boxed error should be a `CryptoError`.
    #[display(fmt = "Crypto error {:?}", _0)]
    CryptoErr(Box<CryptoError>),

    #[display(fmt = "Synchronization {} block error", _0)]
    VerifyTransaction(u64),

    #[display(fmt = "Synchronization/Consensus {} block error : {}", _0, _1)]
    VerifyBlockHeader(u64, BlockHeaderField),

    #[display(fmt = "Synchronization/Consensus {} block error : {}", _0, _1)]
    VerifyProof(u64, BlockProofField),

    /// The Rpc response mismatch the request.
    #[display(fmt = "Synchronization Rpc {:?} message mismatch", _0)]
    RpcErr(ConsensusType),

    ///
    #[display(fmt = "Get merkle root failed {:?}", _0)]
    MerkleErr(String),

    ///
    #[display(fmt = "Execute transactions error {:?}", _0)]
    ExecuteErr(String),

    ///
    WALErr(std::io::Error),

    #[display(fmt = "Storage item not found")]
    StorageItemNotFound,

    /// Other error used for very few errors.
    #[display(fmt = "{:?}", _0)]
    Other(String),
}

#[derive(Debug, Display)]
pub enum BlockHeaderField {
    #[display(fmt = "The pre_hash mismatch the previous block")]
    PreviousBlockHash,

    #[display(fmt = "The pre_hash mismatch the hash in the proof field")]
    ProofHash,

    #[display(fmt = "The proposer is not in the committee")]
    Proposer,

    #[display(fmt = "There is at least one validator not in the committee")]
    Validator,

    #[display(fmt = "There is at least one validator's weight mismatch")]
    Weight,
}

#[derive(Debug, Display)]
pub enum BlockProofField {
    #[display(fmt = "The bit_map has error with committer, can't get signed voters")]
    BitMap,

    #[display(fmt = "The proof signature is fraud or error")]
    Signature,

    #[display(fmt = "Heights of block and proof diverse, block {}, proof {}", _0, _1)]
    HeightMismatch(u64, u64),

    #[display(fmt = "Hash of block and proof diverse")]
    HashMismatch,

    #[display(fmt = "There is at least one validator not in the committee")]
    Validator,

    #[display(fmt = "There is at least one validator's weight mismatch")]
    Weight,

    #[display(fmt = "There is at least one validator's weight missing")]
    WeightNotFound,
}

impl Error for ConsensusError {}

impl From<ConsensusError> for ProtocolError {
    fn from(err: ConsensusError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Consensus, Box::new(err))
    }
}
