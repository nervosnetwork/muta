use crate::{
    proto::{
        common::{Proof, Result as SrvResult},
        consensus::*,
    },
    Context, FutResponse,
};

pub trait ConsensusService {
    fn verify_proof(&self, ctx: Context, proof: Proof) -> FutResponse<SrvResult>;

    fn proc_consensus_message(&self, ctx: Context, msg: Message) -> FutResponse<SrvResult>;

    fn set_status(&self, ctx: Context, state: RichStatus) -> FutResponse<SrvResult>;
}
