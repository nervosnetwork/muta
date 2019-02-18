use futures::future;
use umaru::prelude::*;
use umaru::{
    proto::{blockchain::*, common::*, pool::*},
    service::PoolService,
};

pub struct DummyPool {}

impl PoolService for DummyPool {
    fn add_unverified_transaction(
        &self,
        _ctx: Context,
        utx: UnverifiedTransaction,
    ) -> FutResponse<Result> {
        println!("new unverifiedtransaction: {:?}", utx);

        let mut ret = Result::new();
        ret.set_code((UnverifiedTransactionResult::OK) as u32);

        Box::new(future::ok(ret))
    }

    fn add_batch_unverified_transactions(
        &self,
        _ctx: Context,
        _unverified_transactions: UnverifiedTransactions,
    ) -> FutResponse<Result> {
        unimplemented!()
    }

    fn sync_unverified_transaction_hashes(
        &self,
        _ctx: Context,
        _unverified_transaction_hashes: UnverifiedTransactionHashes,
    ) -> FutResponse<Result> {
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
    ) -> FutResponse<Result> {
        unimplemented!()
    }

    fn check_unverified_sync_block(
        &self,
        _ctx: Context,
        _unverified_sync_block: UnverifiedSyncBlock,
    ) -> FutResponse<Result> {
        unimplemented!()
    }

    fn flush_confirmed_block(
        &self,
        _ctx: Context,
        _confirmed_block: ConfirmedBlock,
    ) -> FutResponse<Result> {
        unimplemented!()
    }
}
