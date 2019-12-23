use protocol::traits::{Executor, ExecutorAdapter, ExecutorParams, ExecutorResp};
use protocol::types::{Address, SignedTransaction, TransactionRequest};
use protocol::ProtocolResult;

use crate::DefaultRequestContext;

pub struct ServiceExecutor<Adapter: ExecutorAdapter> {
    adapter: Adapter,
}

impl<Adapter: ExecutorAdapter> Executor for ServiceExecutor<Adapter> {
    fn exec(
        &mut self,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        unimplemented!()
    }

    fn read(
        &self,
        params: &ExecutorParams,
        caller: &Address,
        request: &TransactionRequest,
    ) -> ProtocolResult<String> {
        // DefaultRequestContext::new(cycles_limit: u64, cycles_price: u64, cycles_used: u64, caller: Address, epoch_id: u64, service_name: String, service_method: String, service_payload: String)
        unimplemented!()
    }
}
