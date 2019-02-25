use core_types::{
    transaction::{SignedTransaction, UnverifiedTransaction},
    Hash,
};

use crate::{Context, FutRuntimeResult};

pub trait Order: Send + Sync {
    type Error;

    fn insert(
        &mut self,
        ctx: &Context,
        signed_tx: SignedTransaction,
    ) -> FutRuntimeResult<(), Self::Error>;

    fn get_batch(&mut self, ctx: &Context, count: u64) -> FutRuntimeResult<&[Hash], Self::Error>;

    fn flush(&mut self, ctx: &Context, hashes: &[&Hash]) -> FutRuntimeResult<(), Self::Error>;

    fn get(
        &mut self,
        ctx: &Context,
        hash: &Hash,
    ) -> FutRuntimeResult<&[&SignedTransaction], Self::Error>;
}

pub trait Verifier {
    type Error;

    fn unverified_transaction(
        &self,
        ctx: &Context,
        untx: UnverifiedTransaction,
    ) -> FutRuntimeResult<SignedTransaction, Self::Error>;
}

// pub trait PoolService {
//     fn add_unverified_transaction(
//         &self,
//         ctx: &Context,
//         unverified_transaction: UnverifiedTransaction,
//     ) -> FutResponse<SrvResult>;
//
//     fn add_batch_unverified_transactions(
//         &self,
//         ctx: &Context,
//         unverified_transactions: UnverifiedTransactions,
//     ) -> FutResponse<SrvResult>;
//
//     fn sync_unverified_transaction_hashes(
//         &self,
//         ctx: &Context,
//         unverified_transaction_hashes: UnverifiedTransactionHashes,
//     ) -> FutResponse<SrvResult>;
//
//     fn pack_unverified_transaction_hashes(
//         &self,
//         ctx: &Context,
//         global_pool_config: GlobalPoolConfig,
//     ) -> FutResponse<UnverifiedTransactionHashesResp>;
//
//     fn check_unverified_proposal_block(
//         &self,
//         ctx: &Context,
//         unverified_proposal_block: UnverifiedProposalBlock,
//     ) -> FutResponse<SrvResult>;
//
//     fn check_unverified_sync_block(
//         &self,
//         ctx: &Context,
//         unverified_sync_block: UnverifiedSyncBlock,
//     ) -> FutResponse<SrvResult>;
//
//     fn flush_confirmed_block(
//         &self,
//         ctx: &Context,
//         confirmed_block: ConfirmedBlock,
//     ) -> FutResponse<SrvResult>;
// }
