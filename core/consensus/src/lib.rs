mod engine;

pub mod adapter;
pub mod consensus;
pub mod fixed_types;
pub mod message;
pub mod util;

pub use overlord::DurationConfig;

use std::error::Error;

use derive_more::{Display, From};

use common_crypto::CryptoError;

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

    #[display(fmt = "Rich Epoch ID")]
    RichEpochID,

    #[display(fmt = "Rpc Pull Epochs")]
    RpcPullEpochs,

    #[display(fmt = "Rpc Pull Transactions")]
    RpcPullTxs,
}

/// Consensus errors defines here.
#[derive(Debug, Display, From)]
pub enum ConsensusError {
    /// Send consensus message error.
    #[display(fmt = "Send {:?} message failed", _0)]
    SendMsgErr(MsgType),

    /// Decode consensus message error.
    #[display(fmt = "Decode {:?} message failed", _0)]
    DecodeErr(MsgType),

    /// Encode consensus message error.
    #[display(fmt = "Encode {:?} message failed", _0)]
    EncodeErr(MsgType),

    /// Overlord consensus protocol error.
    #[display(fmt = "Overlord error {:?}", _0)]
    OverlordErr(Box<dyn Error + Send>),

    /// Consensus missed last epoch proof.
    #[display(fmt = "Consensus missed proof of {} epoch", _0)]
    MissingProof(u64),

    /// Consensus missed the pill.
    #[display(fmt = "Consensus missed pill cooresponding {:?}", _0)]
    MissingPill(Hash),

    /// Consensus missed the epoch header.
    #[display(fmt = "Consensus missed epoch header of {} epoch", _0)]
    MissingEpochHeader(u64),

    /// This boxed error should be a `CryptoError`.
    #[display(fmt = "Crypto error {:?}", _0)]
    CryptoErr(Box<CryptoError>),

    /// The synchronous epoch does not pass the checks.
    #[display(fmt = "Synchronization {} epoch error", _0)]
    SyncEpochHashErr(u64),

    /// The synchronous epoch proof does not pass the checks.
    #[display(fmt = "Synchronization {} proof error", _0)]
    SyncEpochProofErr(u64),

    /// The Rpc response mismatch the request.
    #[display(fmt = "Synchronization Rpc {:?} message mismatch", _0)]
    RpcErr(MsgType),

    ///
    #[display(fmt = "Get merkle root failed {:?}", _0)]
    MerkleErr(String),

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
