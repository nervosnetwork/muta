use std::sync::Arc;

use async_trait::async_trait;
use protocol::types::{
    Address as ProtoAddress, Block, BlockHeader, Bloom, Bytes, FullBlock, Hash as ProtoHash,
    MerkleRoot, Metadata, Pill, Proof as ProtoProof, Receipt, SignedTransaction,
    TransactionRequest, Validator, ValidatorExtend
};
use protocol::traits::{
    Context, MessageHandler, MessageCodec, Priority, Rpc, Storage,
};
use overlord::types::{OverlordMsg, SignedProposal, SignedPreVote, SignedPreCommit, SignedChoke, PreVoteQC, PreCommitQC, SignedHeight, SyncRequest, SyncResponse};
use protocol::{ProtocolResult, ProtocolErrorKind, ProtocolError};

use crate::{WrappedPill, ExecResp, OverlordHandler, ConsensusError};

pub const END_GOSSIP_SIGNED_PROPOSAL: &str = "/gossip/consensus/signed_proposal";
pub const END_GOSSIP_SIGNED_PRE_VOTE: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_SIGNED_PRE_COMMIT: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_PRE_VOTE_QC: &str = "/gossip/consensus/pre_vote_qc";
pub const END_GOSSIP_PRE_COMMIT_QC: &str = "/gossip/consensus/pre_commit_qc";
pub const END_GOSSIP_SIGNED_CHOKE: &str = "/gossip/consensus/signed_choke";
pub const END_GOSSIP_SIGNED_HEIGHT: &str = "/gossip/consensus/signed_height";

pub const RPC_SYNC_PULL_BLOCK_PROOF: &str = "/rpc_call/consensus/sync_pull_block";
pub const RPC_SYNC_PUSH_BLOCK_PROOF: &str = "/rpc_resp/consensus/sync_pull_block";
pub const RPC_SYNC_PULL_TXS: &str = "/rpc_call/consensus/sync_pull_txs";
pub const RPC_SYNC_PUSH_TXS: &str = "/rpc_resp/consensus/sync_pull_txs";

#[derive(Debug)]
pub struct WrappedSignedProposal(SignedProposal<WrappedPill>);
#[derive(Debug)]
pub struct WrappedSignedPreVote(SignedPreVote);
#[derive(Debug)]
pub struct WrappedSignedPreCommit(SignedPreCommit);
#[derive(Debug)]
pub struct WrappedSignedChoke(SignedChoke);
#[derive(Debug)]
pub struct WrappedPreVoteQC(PreVoteQC);
#[derive(Debug)]
pub struct WrappedPreCommitQC(PreCommitQC);
#[derive(Debug)]
pub struct WrappedSignedHeight(SignedHeight);
#[derive(Debug)]
pub struct WrappedSyncRequest(SyncRequest);
#[derive(Debug)]
pub struct WrappedSyncResponse(SyncResponse<WrappedPill>);

#[async_trait]
impl MessageCodec for WrappedSignedProposal{
    async fn encode(&mut self) -> ProtocolResult<Bytes>{
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self>{
        Ok(WrappedSignedProposal(rlp::decode(&bytes).map_err(|e| ConsensusError::MsgDecode)?))
    }
}

pub struct SignedProposalMessageHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> SignedProposalMessageHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for SignedProposalMessageHandler<H> {
    type Message = WrappedSignedProposal;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler.send_msg(ctx, OverlordMsg::SignedProposal(msg.0));
    }
}
