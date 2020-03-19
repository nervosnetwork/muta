use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::traits::{ServiceMapping, Storage};
use crate::types::{
    Address, Bloom, MerkleRoot, Receipt, ServiceContext, SignedTransaction, TransactionRequest,
};
use crate::ProtocolResult;

#[derive(Debug, Clone)]
pub struct ExecutorResp {
    pub receipts:        Vec<Receipt>,
    pub all_cycles_used: u64,
    pub logs_bloom:      Bloom,
    pub state_root:      MerkleRoot,
}

#[derive(Debug, Clone)]
pub struct ExecutorParams {
    pub state_root:   MerkleRoot,
    pub height:       u64,
    pub timestamp:    u64,
    pub cycles_limit: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceResponse<T: Default> {
    pub code:  u64,
    pub data:  T,
    pub error: String,
}

impl<T: Default> ServiceResponse<T> {
    pub fn from_error(code: u64, error: String) -> Self {
        Self {
            code,
            data: T::default(),
            error,
        }
    }

    pub fn from_data(data: T) -> Self {
        Self {
            code: 0,
            data,
            error: "".to_owned(),
        }
    }
}

impl<T: Default + PartialEq> PartialEq for ServiceResponse<T> {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code && self.data == other.data && self.error == other.error
    }
}

impl<T: Default + Eq> Eq for ServiceResponse<T> {}

pub trait ExecutorFactory<DB: cita_trie::DB, S: Storage, Mapping: ServiceMapping>:
    Send + Sync
{
    fn from_root(
        root: MerkleRoot,
        db: Arc<DB>,
        storage: Arc<S>,
        mapping: Arc<Mapping>,
    ) -> ProtocolResult<Box<dyn Executor>>;
}

pub trait Executor {
    fn exec(
        &mut self,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp>;

    fn read(
        &self,
        params: &ExecutorParams,
        caller: &Address,
        cycles_price: u64,
        request: &TransactionRequest,
    ) -> ProtocolResult<ServiceResponse<String>>;
}

// `Dispatcher` provides ability to send a call message to other services
pub trait Dispatcher {
    fn read(&self, context: ServiceContext) -> ServiceResponse<String>;

    fn write(&self, context: ServiceContext) -> ServiceResponse<String>;
}

pub struct NoopDispatcher;

impl Dispatcher for NoopDispatcher {
    fn read(&self, _context: ServiceContext) -> ServiceResponse<String> {
        unimplemented!()
    }

    fn write(&self, _context: ServiceContext) -> ServiceResponse<String> {
        unimplemented!()
    }
}
