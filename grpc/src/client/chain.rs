use crate::{
    grpc::{GrpcChainClient, GrpcChainService},
    proto::blockchain::Block,
    proto::chain::{
        AddBlockResp, BlockHeight, BlockResp, CallResp, Data, ReceiptResp, SignedTransactionResp,
        TransactionHash,
    },
    service::{ChainService, Context, FutResponse},
    ContextExchange, SingleResponseExt,
};

pub struct ChainClient {
    client: GrpcChainClient,
}

impl_client!(ChainClient, CHAIN_CLIENT_HOST, CHAIN_CLIENT_PORT);

impl ChainService for ChainClient {
    fn add_block(&self, ctx: Context, block: Block) -> FutResponse<AddBlockResp> {
        self.client
            .add_block(ctx.into_rpc_context(), block)
            .into_fut_resp()
    }

    fn get_block(&self, ctx: Context, block_height: BlockHeight) -> FutResponse<BlockResp> {
        self.client
            .get_block(ctx.into_rpc_context(), block_height)
            .into_fut_resp()
    }

    fn get_receipt(&self, ctx: Context, tx_hash: TransactionHash) -> FutResponse<ReceiptResp> {
        self.client
            .get_receipt(ctx.into_rpc_context(), tx_hash)
            .into_fut_resp()
    }

    fn get_transaction(
        &self,
        ctx: Context,
        hash: TransactionHash,
    ) -> FutResponse<SignedTransactionResp> {
        self.client
            .get_transaction(ctx.into_rpc_context(), hash)
            .into_fut_resp()
    }

    fn call(&self, ctx: Context, data: Data) -> FutResponse<CallResp> {
        self.client
            .call(ctx.into_rpc_context(), data)
            .into_fut_resp()
    }
}
