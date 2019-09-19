use async_trait::async_trait;

use crate::types::MerkleRoot;
use crate::ProtocolResult;

#[async_trait]
pub trait Executor<Adapter: ExecutorAdapter>: Send + Sync {
    type Adapter: ExecutorAdapter;

    fn exec(&self) -> ProtocolResult<()>;
}

#[async_trait]
pub trait ExecutorAdapter: Send + Sync {
    fn get_epoch_header(&self) -> ProtocolResult<()>;
}

pub trait VM {
    fn call(&self) -> ProtocolResult<()>;
}

pub trait ExecutorState {
    type Key: Clone;
    type Value: Clone;

    fn get(&self, key: &Self::Key) -> ProtocolResult<Option<Self::Value>>;

    fn contains(&self, key: &Self::Key) -> ProtocolResult<bool>;

    fn stash(&mut self) -> ProtocolResult<()>;

    fn revert(&mut self) -> ProtocolResult<()>;

    fn commit(&mut self) -> ProtocolResult<MerkleRoot>;
}
