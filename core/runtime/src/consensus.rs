use std::error::Error;
use std::fmt;

use bft_rs::error::BftError;
use futures::prelude::Future;

use core_context::Context;
use core_crypto::CryptoError;
use core_serialization::CodecError;
use core_types::{Block, Proof, SignedTransaction, TypesError};

use crate::{ExecutorError, StorageError, TransactionPoolError};

pub type FutConsensusResult<T> = Box<dyn Future<Item = T, Error = ConsensusError> + Send>;

pub trait Consensus: Send + Sync {
    fn set_proposal(&self, ctx: Context, msg: Vec<u8>) -> FutConsensusResult<()>;

    fn set_vote(&self, ctx: Context, msg: Vec<u8>) -> FutConsensusResult<()>;

    // Send status to peers after synchronizing blocks to trigger bft
    fn send_status(&self) -> FutConsensusResult<()>;

    /// insert block syncing from other nodes
    fn insert_sync_block(
        &self,
        ctx: Context,
        block: Block,
        stxs: Vec<SignedTransaction>,
        proof: Proof,
    ) -> FutConsensusResult<()>;
}

#[derive(Debug)]
pub enum ConsensusError {
    TransactionPool(TransactionPoolError),
    Executor(ExecutorError),
    Storage(StorageError),
    Crypto(CryptoError),
    Codec(CodecError),
    Types(TypesError),
    Bft(BftError),
    Internal(String),

    InvalidProposal(String),
}

impl Error for ConsensusError {}
impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            ConsensusError::TransactionPool(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Executor(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Storage(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Crypto(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Codec(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Types(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Bft(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Internal(ref err) => format!("consensus: {:?}", err),

            ConsensusError::InvalidProposal(ref err) => format!("consensus: {:?}", err),
        };
        write!(f, "{}", printable)
    }
}

impl From<TransactionPoolError> for ConsensusError {
    fn from(err: TransactionPoolError) -> Self {
        ConsensusError::TransactionPool(err)
    }
}

impl From<ExecutorError> for ConsensusError {
    fn from(err: ExecutorError) -> Self {
        ConsensusError::Executor(err)
    }
}

impl From<StorageError> for ConsensusError {
    fn from(err: StorageError) -> Self {
        ConsensusError::Storage(err)
    }
}

impl From<CryptoError> for ConsensusError {
    fn from(err: CryptoError) -> Self {
        ConsensusError::Crypto(err)
    }
}

impl From<CodecError> for ConsensusError {
    fn from(err: CodecError) -> Self {
        ConsensusError::Codec(err)
    }
}

impl From<TypesError> for ConsensusError {
    fn from(err: TypesError) -> Self {
        ConsensusError::Types(err)
    }
}

impl From<BftError> for ConsensusError {
    fn from(err: BftError) -> Self {
        ConsensusError::Bft(err)
    }
}
