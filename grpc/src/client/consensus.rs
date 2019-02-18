use crate::{
    grpc::{GrpcConsensusClient, GrpcConsensusService},
    proto::{
        blockchain::Proof,
        common::Result as SrvResult,
        consensus::{Message, RichStatus},
    },
    service::{ConsensusService, Context, FutResponse},
    ContextExchange, SingleResponseExt,
};

pub struct ConsensusClient {
    client: GrpcConsensusClient,
}

impl_client!(
    ConsensusClient,
    CONSENSUS_CLIENT_HOST,
    CONSENSUS_CLIENT_PORT
);

impl ConsensusService for ConsensusClient {
    fn verify_proof(&self, ctx: Context, proof: Proof) -> FutResponse<SrvResult> {
        self.client
            .verify_proof(ctx.into_rpc_context(), proof)
            .into_fut_resp()
    }

    fn proc_consensus_message(&self, ctx: Context, msg: Message) -> FutResponse<SrvResult> {
        self.client
            .proc_consensus_message(ctx.into_rpc_context(), msg)
            .into_fut_resp()
    }

    fn set_status(&self, ctx: Context, status: RichStatus) -> FutResponse<SrvResult> {
        self.client
            .set_status(ctx.into_rpc_context(), status)
            .into_fut_resp()
    }
}
