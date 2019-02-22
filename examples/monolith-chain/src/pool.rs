use futures::future;
use muta::proto::blockchain::UnverifiedTransaction;
use muta::proto::common;
use muta::proto::pool::{
    ConfirmedBlock, GlobalPoolConfig, UnverifiedProposalBlock, UnverifiedSyncBlock,
    UnverifiedTransactionHashes, UnverifiedTransactionHashesResp, UnverifiedTransactionResult,
    UnverifiedTransactions,
};
use muta::service::{Context, FutResponse, PoolService};

pub struct DummyPool {}

impl PoolService for DummyPool {
    fn add_unverified_transaction(
        &self,
        _ctx: Context,
        utx: UnverifiedTransaction,
    ) -> FutResponse<common::Result> {
        println!("new unverifiedtransaction: {:?}", utx);

        let mut ret = common::Result::new();
        ret.set_code((UnverifiedTransactionResult::OK) as u32);

        Box::new(future::ok(ret))
    }

    fn add_batch_unverified_transactions(
        &self,
        _ctx: Context,
        _unverified_transactions: UnverifiedTransactions,
    ) -> FutResponse<common::Result> {
        unimplemented!()
    }

    fn sync_unverified_transaction_hashes(
        &self,
        _ctx: Context,
        _unverified_transaction_hashes: UnverifiedTransactionHashes,
    ) -> FutResponse<common::Result> {
        unimplemented!()
    }

    fn pack_unverified_transaction_hashes(
        &self,
        _ctx: Context,
        _global_pool_config: GlobalPoolConfig,
    ) -> FutResponse<UnverifiedTransactionHashesResp> {
        unimplemented!()
    }

    fn check_unverified_proposal_block(
        &self,
        _ctx: Context,
        _unverified_proposal_block: UnverifiedProposalBlock,
    ) -> FutResponse<common::Result> {
        unimplemented!()
    }

    fn check_unverified_sync_block(
        &self,
        _ctx: Context,
        _unverified_sync_block: UnverifiedSyncBlock,
    ) -> FutResponse<common::Result> {
        unimplemented!()
    }

    fn flush_confirmed_block(
        &self,
        _ctx: Context,
        _confirmed_block: ConfirmedBlock,
    ) -> FutResponse<common::Result> {
        unimplemented!()
    }
}
