use crate::{
    grpc::{GrpcSyncClient, GrpcSyncService},
    proto::{common::Result as SrvResult, sync::*},
    service::{Context, FutResponse, SyncService},
    ContextExchange, SingleResponseExt,
};

pub struct SyncClient {
    client: GrpcSyncClient,
}

impl_client!(SyncClient, SYNC_CLIENT_HOST, SYNC_CLIENT_PORT);

impl SyncService for SyncClient {
    fn update_status(&self, ctx: Context, status: Status) -> FutResponse<SrvResult> {
        self.client
            .update_status(ctx.into_rpc_context(), status)
            .into_fut_resp()
    }

    fn proc_sync_request(&self, ctx: Context, req: SyncRequest) -> FutResponse<SyncResp> {
        self.client
            .proc_sync_request(ctx.into_rpc_context(), req)
            .into_fut_resp()
    }

    fn proc_sync_response(&self, ctx: Context, resp: SyncResp) -> FutResponse<SrvResult> {
        self.client
            .proc_sync_response(ctx.into_rpc_context(), resp)
            .into_fut_resp()
    }

    fn get_peer_count(&self, ctx: Context) -> FutResponse<PeerCountResp> {
        self.client
            .get_peer_count(ctx.into_rpc_context(), Default::default())
            .into_fut_resp()
    }
}
