use std::error::Error;
use std::fmt;

use bft_rs::error::BftError;

use core_crypto::CryptoError;
use core_runtime::{ExecutorError, TransactionPoolError};
use core_serialization::CodecError;
use core_storage::StorageError;
use core_types::TypesError;

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

#[derive(Debug)]
pub enum SynchronizerError {
    Internal(String),
    Storage(StorageError),
    Consensus(ConsensusError),
}

impl Error for SynchronizerError {}
impl fmt::Display for SynchronizerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            SynchronizerError::Internal(ref err) => format!("internal error {:?}", err),
            SynchronizerError::Storage(ref err) => format!("storage error {:?}", err),
            SynchronizerError::Consensus(ref err) => format!("consensus error {:?}", err),
        };
        write!(f, "{}", printable)
    }
}

impl From<StorageError> for SynchronizerError {
    fn from(err: StorageError) -> Self {
        SynchronizerError::Storage(err)
    }
}

impl From<ConsensusError> for SynchronizerError {
    fn from(err: ConsensusError) -> Self {
        SynchronizerError::Consensus(err)
    }
}
