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

pub use crate::adapter::OverlordConsensusAdapter;
pub use crate::consensus::OverlordConsensus;
pub use crate::synchronization::OverlordSynchronization;
pub use overlord::{types::Node, DurationConfig};

use std::error::Error;

use derive_more::Display;

use common_crypto::Error as CryptoError;

use protocol::types::Hash;
use protocol::{ProtocolError, ProtocolErrorKind};

#[derive(Clone, Debug, Display, PartialEq, Eq)]
pub enum MsgType {
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
}

/// Consensus errors defines here.
#[derive(Debug, Display)]
pub enum ConsensusError {
    /// Send consensus message error.
    #[display(fmt = "Send {:?} message failed", _0)]
    SendMsgErr(MsgType),

    /// Check block error.
    #[display(fmt = "Check invalid prev_hash, expect {:?} get {:?}", expect, actual)]
    InvalidPrevhash { expect: Hash, actual: Hash },

    #[display(fmt = "Check invalid status vec")]
    InvalidStatusVec,

    /// Decode consensus message error.
    #[display(fmt = "Decode {:?} message failed", _0)]
    DecodeErr(MsgType),

    /// Encode consensus message error.
    #[display(fmt = "Encode {:?} message failed", _0)]
    EncodeErr(MsgType),

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
    SyncBlockHashErr(u64),

    /// The Rpc response mismatch the request.
    #[display(fmt = "Synchronization Rpc {:?} message mismatch", _0)]
    RpcErr(MsgType),

    ///
    #[display(fmt = "Get merkle root failed {:?}", _0)]
    MerkleErr(String),

    ///
    #[display(fmt = "Execute transactions error {:?}", _0)]
    ExecuteErr(String),

    ///
    WalErr(String),

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
