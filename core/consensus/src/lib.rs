#![feature(test)]

pub mod adapter;
pub mod consensus;
mod engine;
pub mod fixed_types;
pub mod message;
pub mod status;
pub mod synchronization;
#[cfg(test)]
mod tests;
pub mod trace;
pub mod util;
pub mod wal;
mod wal_proto;

pub use crate::adapter::OverlordConsensusAdapter;
pub use crate::consensus::OverlordConsensus;
pub use crate::synchronization::{OverlordSynchronization, RichBlock};
pub use crate::wal::SignedTxsWAL;
pub use overlord::{types::Node, DurationConfig};

use std::error::Error;

use derive_more::Display;

use common_crypto::Error as CryptoError;

use protocol::types::Hash;
use protocol::{ProtocolError, ProtocolErrorKind};

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

    /// The synchronous block does not pass the checks.
    #[display(fmt = "Synchronization {} block error", _0)]
    VerifyBlockHeaderPreBlockHash(u64),

    /// The synchronous block does not pass the checks.
    #[display(fmt = "Synchronization {} block prehash error", _0)]
    VerifyBlockHeaderPreHash(u64),

    #[display(
        fmt = "Synchronization {} block error, proposer is not in verify list",
        _0
    )]
    VerifyBlockHeaderProposer(u64),

    /// the validator is not in the verify list
    #[display(
        fmt = "Synchronization {} block error, proposer is not in verify list",
        _0
    )]
    VerifyBlockHeaderValidator(u64),

    /// the validator is in the verify list, but weight is not match
    #[display(
        fmt = "Synchronization {} block error, proposer is not in verify list",
        _0
    )]
    VerifyBlockHeaderValidatorWeight(u64),

    /// The Aggregated Signature doesn't match
    #[display(fmt = "Verify block {} block error, proof doesn't match", _0)]
    VerifyBlockProof(u64),

    /// the block and proof is mismatch, you may pass it wrong
    #[display(
        fmt = "Consensus verify block error, block height {} and proof height {} doesn't match",
        _0,
        _1
    )]
    VerifyBlockProofAndBlockHeightMismatch(u64, u64),

    /// the block and proof is mismatch, you may pass it wrong
    #[display(
        fmt = "Consensus verify block error, block {}, block hash and proof hash doesn't match",
        _0
    )]
    VerifyBlockHashMismatch(u64),

    #[display(
        fmt = "Consensus verify block {} block error, signed voter is not in verifier list",
        _0
    )]
    VerifyBlockProofVoter(u64),

    /// The block vote weight is less or equal than 1/3
    #[display(
        fmt = "Consensus verify block {} block error, weight doesn't exceed 2/3",
        _0
    )]
    VerifyBlockProofVoteWeight(u64),

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

    /// Other error used for very few errors.
    #[display(fmt = "{:?}", _0)]
    Other(String),
}

impl Error for ConsensusError {}

impl From<ConsensusError> for ProtocolError {
    fn from(err: ConsensusError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Consensus, Box::new(err))
    }
}
