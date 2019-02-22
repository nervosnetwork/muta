use crate::{
    proto::blockchain::Block,
    proto::chain::{
        AddBlockResp, BlockHeight, BlockResp, CallResp, Data, ReceiptResp, SignedTransactionResp,
        TransactionHash,
    },
    Context, FutResponse,
};

pub trait ChainService {
    fn add_block(&self, ctx: Context, block: Block) -> FutResponse<AddBlockResp>;

    fn get_block(&self, ctx: Context, block_height: BlockHeight) -> FutResponse<BlockResp>;

    fn get_receipt(&self, ctx: Context, tx_hash: TransactionHash) -> FutResponse<ReceiptResp>;

    fn get_transaction(
        &self,
        ctx: Context,
        hash: TransactionHash,
    ) -> FutResponse<SignedTransactionResp>;

    fn call(&self, ctx: Context, data: Data) -> FutResponse<CallResp>;
}
