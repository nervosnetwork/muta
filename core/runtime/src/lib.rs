#![feature(async_await, try_trait)]

use futures::future::Future;

pub mod consensus;
pub mod database;
pub mod executor;
pub mod network;
pub mod storage;
pub mod sync;
pub mod transaction_pool;

pub use consensus::{Consensus, ConsensusError, FutConsensusResult};
pub use database::{DataCategory, Database, DatabaseError, FutDBResult};
pub use executor::{ExecutionContext, ExecutionResult, Executor, ExecutorError, ReadonlyResult};
pub use storage::{Storage, StorageError, StorageResult};
pub use sync::{SyncStatus, SynchronizerError};
pub use transaction_pool::{TransactionOrigin, TransactionPool, TransactionPoolError};

pub type FutRuntimeResult<T, E> = Box<Future<Item = T, Error = E> + Send>;
