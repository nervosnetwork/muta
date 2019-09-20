pub mod contract;

use async_trait::async_trait;
use bytes::Bytes;

use crate::types::{Address, ContractAddress};
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

#[derive(Clone, Debug)]
pub struct InvokeContext {
    cycle_used: u64,
    caller:     Address,
}

pub trait Dispatcher {
    fn invoke(
        &self,
        ictx: InvokeContext,
        address: ContractAddress,
        method: &str,
        args: Vec<Bytes>,
    ) -> ProtocolResult<Bytes>;
}

pub trait ContractSchema {
    type Key: ContractSer + Clone + std::hash::Hash + PartialEq + Eq + PartialOrd + Ord;
    type Value: ContractSer + Clone;
}

pub trait ContractSer {
    fn encode(&self) -> ProtocolResult<Bytes>;

    fn decode(bytes: Bytes) -> ProtocolResult<Self>
    where
        Self: Sized;
}
