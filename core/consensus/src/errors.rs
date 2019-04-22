use std::error::Error;
use std::fmt;

use core_crypto::CryptoError;
use core_runtime::{ExecutorError, TransactionPoolError};
use core_storage::StorageError;

#[derive(Debug)]
pub enum ConsensusError {
    TransactionPool(TransactionPoolError),
    Executor(ExecutorError),
    Storage(StorageError),
    Crypto(CryptoError),
    Internal(String),

    InvalidBlockTime,
    InvalidQuotaLimit,
    InvalidPrevhash,
    InvalidHeight,
}

impl Error for ConsensusError {}
impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            ConsensusError::TransactionPool(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Executor(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Storage(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Crypto(ref err) => format!("consensus: {:?}", err),
            ConsensusError::Internal(ref err) => format!("consensus: {:?}", err),

            ConsensusError::InvalidBlockTime => "invalid block time".to_owned(),
            ConsensusError::InvalidQuotaLimit => "invalid quota limit".to_owned(),
            ConsensusError::InvalidPrevhash => "invalid prevhash".to_owned(),
            ConsensusError::InvalidHeight => "invalid height".to_owned(),
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
