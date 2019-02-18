use std::marker::{Send, Sync};

use grpc::{RequestOptions, SingleResponse};

use crate::{
    grpc::GrpcConsensusService,
    proto::{blockchain::Proof, common::Result as SrvResult, consensus::*},
    service::{ConsensusService, Context},
    ContextExchange, FutResponseExt,
};

pub struct ConsensusServer {
    server: ::grpc::Server,
}

impl_server!(ConsensusServer, GrpcConsensusImpl, CONSENSUS);

struct GrpcConsensusImpl<T> {
    core_srv: T,
}

impl<T: ConsensusService + Sync + Send + 'static> GrpcConsensusService for GrpcConsensusImpl<T> {
    fn verify_proof(&self, o: RequestOptions, proof: Proof) -> SingleResponse<SrvResult> {
        self.core_srv
            .verify_proof(Context::from_rpc_context(o), proof)
            .into_single_resp()
    }

    fn proc_consensus_message(&self, o: RequestOptions, msg: Message) -> SingleResponse<SrvResult> {
        self.core_srv
            .proc_consensus_message(Context::from_rpc_context(o), msg)
            .into_single_resp()
    }

    fn set_status(&self, o: RequestOptions, status: RichStatus) -> SingleResponse<SrvResult> {
        self.core_srv
            .set_status(Context::from_rpc_context(o), status)
            .into_single_resp()
    }
}
