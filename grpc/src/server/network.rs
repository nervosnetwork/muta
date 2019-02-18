use std::marker::{Send, Sync};

use grpc::{RequestOptions, SingleResponse};

use crate::{
    grpc::GrpcNetworkService,
    proto::{
        blockchain::UnverifiedTransaction, common::Result as SrvResult, consensus, pool, sync,
    },
    service::{Context, NetworkService},
    ContextExchange, FutResponseExt,
};

pub struct NetworkServer {
    server: ::grpc::Server,
}

impl_server!(NetworkServer, GrpcNetworkImpl, NETWORK);

struct GrpcNetworkImpl<T> {
    core_srv: T,
}

impl<T: NetworkService + Sync + Send + 'static> GrpcNetworkService for GrpcNetworkImpl<T> {
    fn forward_unverified_transaction(
        &self,
        o: RequestOptions,
        utx: UnverifiedTransaction,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .forward_unverified_transaction(Context::from_rpc_context(o), utx)
            .into_single_resp()
    }

    fn send_unverified_transaction_hashes(
        &self,
        o: RequestOptions,
        utx_hashes: pool::UnverifiedTransactionHashes,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .send_unverified_transaction_hashes(Context::from_rpc_context(o), utx_hashes)
            .into_single_resp()
    }

    fn send_unverified_transactions(
        &self,
        o: RequestOptions,
        utx: pool::UnverifiedTransactions,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .send_unverified_transactions(Context::from_rpc_context(o), utx)
            .into_single_resp()
    }

    fn broadcast_consensus_message(
        &self,
        o: RequestOptions,
        msg: consensus::Message,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .broadcast_consensus_message(Context::from_rpc_context(o), msg)
            .into_single_resp()
    }

    fn broadcast_new_status(
        &self,
        o: RequestOptions,
        status: sync::Status,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .broadcast_new_status(Context::from_rpc_context(o), status)
            .into_single_resp()
    }

    fn send_sync_request(
        &self,
        o: RequestOptions,
        req: sync::SyncRequest,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .send_sync_request(Context::from_rpc_context(o), req)
            .into_single_resp()
    }

    fn send_sync_response(
        &self,
        o: RequestOptions,
        resp: sync::SyncResp,
    ) -> SingleResponse<SrvResult> {
        self.core_srv
            .send_sync_response(Context::from_rpc_context(o), resp)
            .into_single_resp()
    }
}
