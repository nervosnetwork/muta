use crate::types::{Address, Bloom, MerkleRoot, Receipt, SignedTransaction, TransactionRequest};
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
    pub epoch_id:     u64,
    pub timestamp:    u64,
    pub cycels_limit: u64,
}

#[derive(Debug, Clone)]
pub struct ExecResp {
    pub ret:      String,
    pub is_error: bool,
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
