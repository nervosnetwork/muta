mod consensus;

use std::error::Error;

use derive_more::{Display, From};

use protocol::{ProtocolError, ProtocolErrorKind};

#[derive(Clone, Debug, Display, PartialEq, Eq)]
pub enum ConsensusMsg {
    #[display(fmt = "Signed Proposal")]
    SignedProposal,
    #[display(fmt = "Signed Vote")]
    SignedVote,
    #[display(fmt = "Aggregated Vote")]
    AggregateVote,
}

/// Consensus errors defines here.
#[derive(Debug, Display, From)]
pub enum ConsensusError {
    /// Send consensus error.
    #[display(fmt = "Send {:?} message failed", _0)]
    SendMsgErr(ConsensusMsg),

    /// This boxed error should be a `CryptoError`.
    #[display(fmt = "Crypto error {:?}", _0)]
    CryptoErr(Box<dyn Error + Send>),

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
