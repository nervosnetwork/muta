use std::marker::{Send, Sync};

use grpc::{RequestOptions, SingleResponse};

use crate::{
    grpc::GrpcSyncService,
    proto::{common, sync::*},
    service::{Context, SyncService},
    ContextExchange, FutResponseExt,
};

pub struct SyncServer {
    server: ::grpc::Server,
}

impl_server!(SyncServer, GrpcSyncImpl, SYNC);

struct GrpcSyncImpl<T> {
    core_srv: T,
}

impl<T: SyncService + Sync + Send + 'static> GrpcSyncService for GrpcSyncImpl<T> {
    fn update_status(&self, o: RequestOptions, status: Status) -> SingleResponse<common::Result> {
        self.core_srv
            .update_status(Context::from_rpc_context(o), status)
            .into_single_resp()
    }

    fn proc_sync_request(&self, o: RequestOptions, req: SyncRequest) -> SingleResponse<SyncResp> {
        self.core_srv
            .proc_sync_request(Context::from_rpc_context(o), req)
            .into_single_resp()
    }

    fn proc_sync_response(
        &self,
        o: RequestOptions,
        resp: SyncResp,
    ) -> SingleResponse<common::Result> {
        self.core_srv
            .proc_sync_response(Context::from_rpc_context(o), resp)
            .into_single_resp()
    }

    fn get_peer_count(&self, o: RequestOptions, _: common::Empty) -> SingleResponse<PeerCountResp> {
        self.core_srv
            .get_peer_count(Context::from_rpc_context(o))
            .into_single_resp()
    }
}
