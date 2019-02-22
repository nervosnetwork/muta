use crate::{
    grpc::{GrpcPoolClient, GrpcPoolService},
    proto::pool::{
        ConfirmedBlock, GlobalPoolConfig, UnverifiedProposalBlock, UnverifiedSyncBlock,
        UnverifiedTransactionHashes, UnverifiedTransactionHashesResp, UnverifiedTransactions,
    },
    proto::{blockchain::UnverifiedTransaction, common::Result as SrvResult},
    service::{Context, FutResponse, PoolService},
    ContextExchange, SingleResponseExt,
};

pub struct PoolClient {
    client: GrpcPoolClient,
}

impl_client!(PoolClient, POOL_CLIENT_HOST, POOL_CLIENT_PORT);

impl PoolService for PoolClient {
    fn add_unverified_transaction(
        &self,
        ctx: Context,
        utx: UnverifiedTransaction,
    ) -> FutResponse<SrvResult> {
        self.client
            .add_unverified_transaction(ctx.into_rpc_context(), utx)
            .into_fut_resp()
    }

    fn add_batch_unverified_transactions(
        &self,
        ctx: Context,
        utxs: UnverifiedTransactions,
    ) -> FutResponse<SrvResult> {
        self.client
            .add_batch_unverified_transactions(ctx.into_rpc_context(), utxs)
            .into_fut_resp()
    }

    fn sync_unverified_transaction_hashes(
        &self,
        ctx: Context,
        hashes: UnverifiedTransactionHashes,
    ) -> FutResponse<SrvResult> {
        self.client
            .sync_unverified_transaction_hashes(ctx.into_rpc_context(), hashes)
            .into_fut_resp()
    }

    fn pack_unverified_transaction_hashes(
        &self,
        ctx: Context,
        config: GlobalPoolConfig,
    ) -> FutResponse<UnverifiedTransactionHashesResp> {
        self.client
            .pack_unverified_transaction_hashes(ctx.into_rpc_context(), config)
            .into_fut_resp()
    }

    fn check_unverified_proposal_block(
        &self,
        ctx: Context,
        proposal: UnverifiedProposalBlock,
    ) -> FutResponse<SrvResult> {
        self.client
            .check_unverified_proposal_block(ctx.into_rpc_context(), proposal)
            .into_fut_resp()
    }

    fn check_unverified_sync_block(
        &self,
        ctx: Context,
        block: UnverifiedSyncBlock,
    ) -> FutResponse<SrvResult> {
        self.client
            .check_unverified_sync_block(ctx.into_rpc_context(), block)
            .into_fut_resp()
    }

    fn flush_confirmed_block(&self, ctx: Context, block: ConfirmedBlock) -> FutResponse<SrvResult> {
        self.client
            .flush_confirmed_block(ctx.into_rpc_context(), block)
            .into_fut_resp()
    }
}
