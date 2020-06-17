use std::sync::Arc;

use creep::Context;

use crate::traits::{ServiceMapping, Storage};
use crate::types::{
    Address, MerkleRoot, Receipt, ServiceContext, SignedTransaction, TransactionRequest,
};
use crate::ProtocolResult;

#[derive(Debug, Clone)]
pub struct ExecutorResp {
    pub receipts:        Vec<Receipt>,
    pub all_cycles_used: u64,
    pub state_root:      MerkleRoot,
}

#[derive(Debug, Clone)]
pub struct ExecutorParams {
    pub state_root:   MerkleRoot,
    pub height:       u64,
    pub timestamp:    u64,
    pub cycles_limit: u64,
    pub proposer:     Address,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceResponse<T: Default> {
    pub code:          u64,
    pub succeed_data:  T,
    pub error_message: String,
}

impl<T: Default> ServiceResponse<T> {
    pub fn from_error(code: u64, error_message: String) -> Self {
        Self {
            code,
            succeed_data: T::default(),
            error_message,
        }
    }

    pub fn from_succeed(succeed_data: T) -> Self {
        Self {
            code: 0,
            succeed_data,
            error_message: "".to_owned(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.code != 0
    }
}

impl<T: Default + PartialEq> PartialEq for ServiceResponse<T> {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
            && self.succeed_data == other.succeed_data
            && self.error_message == other.error_message
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
        ctx: Context,
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
