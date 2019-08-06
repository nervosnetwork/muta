use async_trait::async_trait;

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
