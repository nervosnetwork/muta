use std::marker::{Send, Sync};

use grpc::{RequestOptions, SingleResponse};

use crate::{
    grpc::GrpcPoolService,
    proto::pool::{
        ConfirmedBlock, GlobalPoolConfig, UnverifiedProposalBlock, UnverifiedSyncBlock,
        UnverifiedTransactionHashes, UnverifiedTransactionHashesResp, UnverifiedTransactions,
    },
    proto::{blockchain::UnverifiedTransaction, common::Result as SrvResult},
    service::{Context, PoolService},
    ContextExchange, FutResponseExt,
};

pub struct PoolServer {
    server: ::grpc::Server,
}

impl_server!(PoolServer, GrpcPoolImpl, POOL);

struct GrpcPoolImpl<T> {
    core_srv: T,
}

impl<T: PoolService + Sync + Send + 'static> GrpcPoolService for GrpcPoolImpl<T> {
    fn add_unverified_transaction(
        &self,
        o: RequestOptions,
        utx: UnverifiedTransaction,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .add_unverified_transaction(Context::from_rpc_context(o), utx)
            .into_single_resp()
    }

    fn add_batch_unverified_transactions(
        &self,
        o: RequestOptions,
        utxs: UnverifiedTransactions,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .add_batch_unverified_transactions(Context::from_rpc_context(o), utxs)
            .into_single_resp()
    }

    fn sync_unverified_transaction_hashes(
        &self,
        o: RequestOptions,
        hashes: UnverifiedTransactionHashes,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .sync_unverified_transaction_hashes(Context::from_rpc_context(o), hashes)
            .into_single_resp()
    }

    fn pack_unverified_transaction_hashes(
        &self,
        o: RequestOptions,
        config: GlobalPoolConfig,
    ) -> SingleResponse<UnverifiedTransactionHashesResp> {
        self.core_srv
            .pack_unverified_transaction_hashes(Context::from_rpc_context(o), config)
            .into_single_resp()
    }

    fn check_unverified_proposal_block(
        &self,
        o: RequestOptions,
        proposal: UnverifiedProposalBlock,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .check_unverified_proposal_block(Context::from_rpc_context(o), proposal)
            .into_single_resp()
    }

    fn check_unverified_sync_block(
        &self,
        o: RequestOptions,
        block: UnverifiedSyncBlock,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .check_unverified_sync_block(Context::from_rpc_context(o), block)
            .into_single_resp()
    }

    fn flush_confirmed_block(
        &self,
        o: RequestOptions,
        block: ConfirmedBlock,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .flush_confirmed_block(Context::from_rpc_context(o), block)
            .into_single_resp()
    }
}
