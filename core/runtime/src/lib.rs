use futures::future::Future;

pub mod database;
pub mod events;
// pub mod consensus;
pub mod executor;
// pub mod network;
pub mod pool;
// pub mod sync;

pub use database::{Database, DatabaseError};
pub use events::EventType;
pub use pool::{Order, Verifier};

pub type FutRuntimeResult<T, E> = Box<Future<Item = T, Error = E>>;

/// Blockchain Context. eg. block, system contract.
// TODO: Add context information
#[derive(Default)]
pub struct Context {}
