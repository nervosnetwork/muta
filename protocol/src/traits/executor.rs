use crate::traits::{Service, ServiceSDK};
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
    state_root:   MerkleRoot,
    epoch_id:     u64,
    cycels_limit: u64,
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
        request: &TransactionRequest,
    ) -> ProtocolResult<String>;
}

pub trait ExecutorAdapter {
    fn get_service_inst<SDK: ServiceSDK, Inst: Service<SDK>>(
        &self,
        service_name: &str,
    ) -> ProtocolResult<Inst>;

    fn revert_state(&mut self) -> ProtocolResult<()>;

    fn stash_state(&mut self) -> ProtocolResult<()>;

    fn commit_state(&mut self) -> ProtocolResult<MerkleRoot>;
}
