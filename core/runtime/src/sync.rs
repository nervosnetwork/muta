use std::error::Error;
use std::fmt;

use core_context::Context;
use core_types::{Block, Hash, SignedTransaction};

use crate::{BoxFuture, ConsensusError, StorageError};

pub type FutSyncResult<T> = BoxFuture<'static, Result<T, SynchronizerError>>;

pub trait Synchronization: Send + Sync {
    fn sync_blocks(&self, ctx: Context, global_height: u64) -> FutSyncResult<()>;

    fn get_blocks(&self, ctx: Context, heights: Vec<u64>) -> FutSyncResult<Vec<Block>>;

    fn get_stxs(&self, ctx: Context, hashes: Vec<Hash>) -> FutSyncResult<Vec<SignedTransaction>>;
}

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
    SynchLocked,
}

impl Error for SynchronizerError {}
impl fmt::Display for SynchronizerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            SynchronizerError::Internal(ref err) => format!("internal error {:?}", err),
            SynchronizerError::Storage(ref err) => format!("storage error {:?}", err),
            SynchronizerError::Consensus(ref err) => format!("consensus error {:?}", err),
            SynchronizerError::SynchLocked => "locked in synchronizing blocks".to_owned(),
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
