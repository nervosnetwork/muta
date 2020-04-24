use std::sync::Arc;

use async_trait::async_trait;
use overlord::types::{
    OverlordMsg, PreCommitQC, PreVoteQC, SignedChoke, SignedHeight, SignedPreCommit, SignedPreVote,
    SignedProposal, SyncRequest, SyncResponse,
};
use protocol::traits::{Context, MessageCodec, MessageHandler};
use protocol::types::Bytes;
use protocol::ProtocolResult;

use crate::{ConsensusError, OverlordHandler, WrappedPill};

pub const END_GOSSIP_SIGNED_PROPOSAL: &str = "/gossip/consensus/signed_proposal";
pub const END_GOSSIP_SIGNED_PRE_VOTE: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_SIGNED_PRE_COMMIT: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_PRE_VOTE_QC: &str = "/gossip/consensus/pre_vote_qc";
pub const END_GOSSIP_PRE_COMMIT_QC: &str = "/gossip/consensus/pre_commit_qc";
pub const END_GOSSIP_SIGNED_CHOKE: &str = "/gossip/consensus/signed_choke";
pub const END_GOSSIP_SIGNED_HEIGHT: &str = "/gossip/consensus/signed_height";
pub const END_GOSSIP_SYNC_REQUEST: &str = "/gossip/consensus/sync_request";
pub const END_GOSSIP_SYNC_RESPONSE: &str = "/gossip/consensus/sync_response";

#[derive(Debug)]
pub struct WrappedSignedProposal(SignedProposal<WrappedPill>);

#[async_trait]
impl MessageCodec for WrappedSignedProposal {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedSignedProposal(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct SignedProposalHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> SignedProposalHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for SignedProposalHandler<H> {
    type Message = WrappedSignedProposal;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler
            .send_msg(ctx, OverlordMsg::SignedProposal(msg.0));
    }
}

#[derive(Debug)]
pub struct WrappedSignedPreVote(SignedPreVote);

#[async_trait]
impl MessageCodec for WrappedSignedPreVote {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedSignedPreVote(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct SignedPreVoteHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> SignedPreVoteHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for SignedPreVoteHandler<H> {
    type Message = WrappedSignedPreVote;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler
            .send_msg(ctx, OverlordMsg::SignedPreVote(msg.0));
    }
}

#[derive(Debug)]
pub struct WrappedSignedPreCommit(SignedPreCommit);

#[async_trait]
impl MessageCodec for WrappedSignedPreCommit {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedSignedPreCommit(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct SignedPreCommitHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> SignedPreCommitHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for SignedPreCommitHandler<H> {
    type Message = WrappedSignedPreCommit;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler
            .send_msg(ctx, OverlordMsg::SignedPreCommit(msg.0));
    }
}

#[derive(Debug)]
pub struct WrappedSignedChoke(SignedChoke);

#[async_trait]
impl MessageCodec for WrappedSignedChoke {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedSignedChoke(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct SignedChokeHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> SignedChokeHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for SignedChokeHandler<H> {
    type Message = WrappedSignedChoke;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler.send_msg(ctx, OverlordMsg::SignedChoke(msg.0));
    }
}

#[derive(Debug)]
pub struct WrappedPreVoteQC(PreVoteQC);

#[async_trait]
impl MessageCodec for WrappedPreVoteQC {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedPreVoteQC(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct PreVoteQCHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> PreVoteQCHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for PreVoteQCHandler<H> {
    type Message = WrappedPreVoteQC;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler.send_msg(ctx, OverlordMsg::PreVoteQC(msg.0));
    }
}

#[derive(Debug)]
pub struct WrappedPreCommitQC(PreCommitQC);

#[async_trait]
impl MessageCodec for WrappedPreCommitQC {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedPreCommitQC(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct PreCommitQCHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> PreCommitQCHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for PreCommitQCHandler<H> {
    type Message = WrappedPreCommitQC;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler.send_msg(ctx, OverlordMsg::PreCommitQC(msg.0));
    }
}

#[derive(Debug)]
pub struct WrappedSignedHeight(SignedHeight);

#[async_trait]
impl MessageCodec for WrappedSignedHeight {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedSignedHeight(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct SignedHeightHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> SignedHeightHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for SignedHeightHandler<H> {
    type Message = WrappedSignedHeight;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler.send_msg(ctx, OverlordMsg::SignedHeight(msg.0));
    }
}

#[derive(Debug)]
pub struct WrappedSyncRequest(SyncRequest);

#[async_trait]
impl MessageCodec for WrappedSyncRequest {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedSyncRequest(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct SyncRequestHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> SyncRequestHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for SyncRequestHandler<H> {
    type Message = WrappedSyncRequest;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler.send_msg(ctx, OverlordMsg::SyncRequest(msg.0));
    }
}
#[derive(Debug)]
pub struct WrappedSyncResponse(SyncResponse<WrappedPill>);

#[async_trait]
impl MessageCodec for WrappedSyncResponse {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(&self.0)))
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(WrappedSyncResponse(
            rlp::decode(&bytes).map_err(|_| ConsensusError::MsgDecode)?,
        ))
    }
}

pub struct SyncResponseHandler<H> {
    handler: Arc<H>,
}

impl<H: OverlordHandler + Sync + Send + 'static> SyncResponseHandler<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<H: OverlordHandler + Sync + Send + 'static> MessageHandler for SyncResponseHandler<H> {
    type Message = WrappedSyncResponse;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        self.handler.send_msg(ctx, OverlordMsg::SyncResponse(msg.0));
    }
}
