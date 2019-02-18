use crate::{
    proto::{common::Result as SrvResult, sync::*},
    Context, FutResponse,
};

pub trait SyncService {
    fn update_status(&self, ctx: Context, req: Status) -> FutResponse<SrvResult>;

    fn proc_sync_request(&self, ctx: Context, req: SyncRequest) -> FutResponse<SyncResp>;

    fn proc_sync_response(&self, ctx: Context, resp: SyncResp) -> FutResponse<SrvResult>;

    fn get_peer_count(&self, ctx: Context) -> FutResponse<PeerCountResp>;
}
