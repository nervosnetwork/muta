use crate::{
    grpc::{GrpcNetworkClient, GrpcNetworkService},
    proto::{
        blockchain::UnverifiedTransaction, common::Result as SrvResult, consensus, pool, sync,
    },
    service::{Context, FutResponse, NetworkService},
    ContextExchange, SingleResponseExt,
};

pub struct NetworkClient {
    client: GrpcNetworkClient,
}

impl_client!(NetworkClient, NETWORK_CLIENT_HOST, NETWORK_CLIENT_PORT);

impl NetworkService for NetworkClient {
    fn forward_unverified_transaction(
        &self,
        ctx: Context,
        utx: UnverifiedTransaction,
    ) -> FutResponse<SrvResult> {
        self.client
            .forward_unverified_transaction(ctx.into_rpc_context(), utx)
            .into_fut_resp()
    }

    fn send_unverified_transaction_hashes(
        &self,
        ctx: Context,
        hashes: pool::UnverifiedTransactionHashes,
    ) -> FutResponse<SrvResult> {
        self.client
            .send_unverified_transaction_hashes(ctx.into_rpc_context(), hashes)
            .into_fut_resp()
    }

    fn send_unverified_transactions(
        &self,
        ctx: Context,
        txs: pool::UnverifiedTransactions,
    ) -> FutResponse<SrvResult> {
        self.client
            .send_unverified_transactions(ctx.into_rpc_context(), txs)
            .into_fut_resp()
    }

    fn broadcast_consensus_message(
        &self,
        ctx: Context,
        msg: consensus::Message,
    ) -> FutResponse<SrvResult> {
        self.client
            .broadcast_consensus_message(ctx.into_rpc_context(), msg)
            .into_fut_resp()
    }

    fn broadcast_new_status(&self, ctx: Context, status: sync::Status) -> FutResponse<SrvResult> {
        self.client
            .broadcast_new_status(ctx.into_rpc_context(), status)
            .into_fut_resp()
    }

    fn send_sync_request(&self, ctx: Context, req: sync::SyncRequest) -> FutResponse<SrvResult> {
        self.client
            .send_sync_request(ctx.into_rpc_context(), req)
            .into_fut_resp()
    }

    fn send_sync_response(&self, ctx: Context, resp: sync::SyncResp) -> FutResponse<SrvResult> {
        self.client
            .send_sync_response(ctx.into_rpc_context(), resp)
            .into_fut_resp()
    }
}
