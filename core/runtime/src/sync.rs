use std::error::Error;
use std::fmt;

use core_types::Hash;

use crate::{ConsensusError, StorageError};

#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub hash:   Hash,
    pub height: u64,
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
