use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use futures::future;
use lazy_static::lazy_static;

use muta::prelude::*;
use muta::{
    proto::{blockchain, common, pool::*},
    server::PoolServer,
    service::PoolService,
};

lazy_static! {
    static ref Count: AtomicUsize = AtomicUsize::new(0);
}

struct PoolServiceImpl {
    count: &'static Count,
}

impl PoolService for PoolServiceImpl {
    fn add_unverified_transaction(
        &self,
        ctx: Context,
        utx: blockchain::UnverifiedTransaction,
    ) -> FutResponse<common::Result> {
        println!("received new unverified transaction");
        println!("{:?}", utx);

        let mut resp = common::Result::new();
        resp.set_code((UnverifiedTransactionResult::BAD_SIG) as u32);
        let count = self.count.load(Ordering::SeqCst);
        println!("count: {}", count);
        self.count.store(count + 1, Ordering::SeqCst);

        println!("grpc context: {:?}", ctx);
        Box::new(future::ok(resp))
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

fn main() {
    let server = PoolServer::new(PoolServiceImpl { count: &Count }).unwrap();

    println!("{:?}", server.local_addr());
    loop {
        thread::park();
    }
}
