use crate::{
    proto::{blockchain, common::Result as SrvResult, consensus, pool, sync},
    Context, FutResponse,
};

pub trait NetworkService {
    fn forward_unverified_transaction(
        &self,
        ctx: Context,
        utx: blockchain::UnverifiedTransaction,
    ) -> FutResponse<SrvResult>;

    fn send_unverified_transaction_hashes(
        &self,
        ctx: Context,
        hashes: pool::UnverifiedTransactionHashes,
    ) -> FutResponse<SrvResult>;

    fn send_unverified_transactions(
        &self,
        ctx: Context,
        utx: pool::UnverifiedTransactions,
    ) -> FutResponse<SrvResult>;

    fn broadcast_consensus_message(
        &self,
        ctx: Context,
        consensus_msg: consensus::Message,
    ) -> FutResponse<SrvResult>;

    fn broadcast_new_status(&self, ctx: Context, status: sync::Status) -> FutResponse<SrvResult>;

    fn send_sync_request(&self, ctx: Context, req: sync::SyncRequest) -> FutResponse<SrvResult>;

    fn send_sync_response(&self, ctx: Context, resp: sync::SyncResp) -> FutResponse<SrvResult>;
}
