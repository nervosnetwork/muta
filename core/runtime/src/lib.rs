#![feature(async_await, await_macro, futures_api)]

use futures::future::Future;

pub mod database;
// pub mod consensus;
pub mod executor;
// pub mod network;
pub mod transaction_pool;
// pub mod sync;

pub use database::{DataCategory, Database, DatabaseError, FutDBResult};
pub use executor::{ExecutionResult, Executor, ExecutorError, ReadonlyResult};
pub use transaction_pool::{TransactionPool, TransactionPoolError};

pub type FutRuntimeResult<T, E> = Box<Future<Item = T, Error = E> + Send>;
