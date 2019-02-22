use crate::{
    proto::blockchain::UnverifiedTransaction,
    proto::common::Result as SrvResult,
    proto::pool::{
        ConfirmedBlock, GlobalPoolConfig, UnverifiedProposalBlock, UnverifiedSyncBlock,
        UnverifiedTransactionHashes, UnverifiedTransactionHashesResp, UnverifiedTransactions,
    },
    Context, FutResponse,
};

pub trait PoolService {
    fn add_unverified_transaction(
        &self,
        ctx: Context,
        unverified_transaction: UnverifiedTransaction,
    ) -> FutResponse<SrvResult>;

    fn add_batch_unverified_transactions(
        &self,
        ctx: Context,
        unverified_transactions: UnverifiedTransactions,
    ) -> FutResponse<SrvResult>;

    fn sync_unverified_transaction_hashes(
        &self,
        ctx: Context,
        unverified_transaction_hashes: UnverifiedTransactionHashes,
    ) -> FutResponse<SrvResult>;

    fn pack_unverified_transaction_hashes(
        &self,
        ctx: Context,
        global_pool_config: GlobalPoolConfig,
    ) -> FutResponse<UnverifiedTransactionHashesResp>;

    fn check_unverified_proposal_block(
        &self,
        ctx: Context,
        unverified_proposal_block: UnverifiedProposalBlock,
    ) -> FutResponse<SrvResult>;

    fn check_unverified_sync_block(
        &self,
        ctx: Context,
        unverified_sync_block: UnverifiedSyncBlock,
    ) -> FutResponse<SrvResult>;

    fn flush_confirmed_block(
        &self,
        ctx: Context,
        confirmed_block: ConfirmedBlock,
    ) -> FutResponse<SrvResult>;
}
