use std::sync::Arc;

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
    pub height:     u64,
    pub timestamp:    u64,
    pub cycles_limit: u64,
}

#[derive(Debug, Clone)]
pub struct ExecResp {
    pub ret:      String,
    pub is_error: bool,
}

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
    ) -> ProtocolResult<ExecResp>;
}

// `Dispatcher` provides ability to send a call message to other services
pub trait Dispatcher {
    fn read(&self, context: ServiceContext) -> ProtocolResult<ExecResp>;

    fn write(&self, context: ServiceContext) -> ProtocolResult<ExecResp>;
}

pub struct NoopDispatcher;

impl Dispatcher for NoopDispatcher {
    fn read(&self, _context: ServiceContext) -> ProtocolResult<ExecResp> {
        unimplemented!()
    }

    fn write(&self, _context: ServiceContext) -> ProtocolResult<ExecResp> {
        unimplemented!()
    }
}
