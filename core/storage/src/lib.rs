#![feature(async_await, await_macro, futures_api, try_trait)]

mod errors;
mod storage;

pub use errors::StorageError;
pub use storage::{BlockStorage, Storage, StorageResult};
