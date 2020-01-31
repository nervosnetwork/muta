use std::sync::Arc;

use async_trait::async_trait;
use bincode::serialize;
use log::debug;
use overlord::types::{AggregatedVote, SignedProposal, SignedVote};
use overlord::Codec;
use rlp::Encodable;
use serde::{Deserialize, Serialize};

use protocol::traits::{
    Consensus, Context, MessageHandler, Priority, Rpc, Storage, Synchronization,
};
use protocol::ProtocolResult;

use crate::fixed_types::{FixedBlock, FixedHeight, FixedSignedTxs, PullTxsRequest};

pub const END_GOSSIP_SIGNED_PROPOSAL: &str = "/gossip/consensus/signed_proposal";
pub const END_GOSSIP_SIGNED_VOTE: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_AGGREGATED_VOTE: &str = "/gossip/consensus/qc";
pub const RPC_SYNC_PULL_BLOCK: &str = "/rpc_call/consensus/sync_pull_block";
pub const RPC_RESP_SYNC_PULL_BLOCK: &str = "/rpc_resp/consensus/sync_pull_block";
pub const RPC_SYNC_PULL_TXS: &str = "/rpc_call/consensus/sync_pull_txs";
pub const RPC_RESP_SYNC_PULL_TXS: &str = "/rpc_resp/consensus/sync_pull_txs";
pub const BROADCAST_HEIGHT: &str = "/gossip/consensus/broadcast_height";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proposal(pub Vec<u8>);

impl<C: Codec> From<SignedProposal<C>> for Proposal {
    fn from(proposal: SignedProposal<C>) -> Self {
        Proposal(proposal.rlp_bytes())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Vote(pub Vec<u8>);

impl From<SignedVote> for Vote {
    fn from(vote: SignedVote) -> Self {
        Vote(vote.rlp_bytes())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QC(pub Vec<u8>);

impl From<AggregatedVote> for QC {
    fn from(aggregated_vote: AggregatedVote) -> Self {
        QC(aggregated_vote.rlp_bytes())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RichHeight(pub Vec<u8>);

impl From<FixedHeight> for RichHeight {
    fn from(id: FixedHeight) -> Self {
        RichHeight(serialize(&id).unwrap())
    }
}

pub struct ProposalMessageHandler<C> {
    consensus: Arc<C>,
}

impl<C: Consensus + 'static> ProposalMessageHandler<C> {
    pub fn new(consensus: Arc<C>) -> Self {
        Self { consensus }
    }
}

#[async_trait]
impl<C: Consensus + 'static> MessageHandler for ProposalMessageHandler<C> {
    type Message = Proposal;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        self.consensus.set_proposal(ctx, msg.0).await
    }
}

pub struct VoteMessageHandler<C> {
    consensus: Arc<C>,
}

impl<C: Consensus + 'static> VoteMessageHandler<C> {
    pub fn new(consensus: Arc<C>) -> Self {
        Self { consensus }
    }
}

#[async_trait]
impl<C: Consensus + 'static> MessageHandler for VoteMessageHandler<C> {
    type Message = Vote;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        self.consensus.set_vote(ctx, msg.0).await
    }
}

pub struct QCMessageHandler<C> {
    consensus: Arc<C>,
}

impl<C: Consensus + 'static> QCMessageHandler<C> {
    pub fn new(consensus: Arc<C>) -> Self {
        Self { consensus }
    }
}

#[async_trait]
impl<C: Consensus + 'static> MessageHandler for QCMessageHandler<C> {
    type Message = QC;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        self.consensus.set_qc(ctx, msg.0).await
    }
}

pub struct RemoteHeightMessageHandler<Sy> {
    synchronization: Arc<Sy>,
}

impl<Sy: Synchronization + 'static> RemoteHeightMessageHandler<Sy> {
    pub fn new(synchronization: Arc<Sy>) -> Self {
        Self { synchronization }
    }
}

#[async_trait]
impl<Sy: Synchronization + 'static> MessageHandler for RemoteHeightMessageHandler<Sy> {
    type Message = u64;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        self.synchronization.receive_remote_block(ctx, msg).await
    }
}

#[derive(Debug)]
pub struct PullBlockRpcHandler<R, S> {
    rpc:     Arc<R>,
    storage: Arc<S>,
}

impl<R, S> PullBlockRpcHandler<R, S>
where
    R: Rpc + 'static,
    S: Storage + 'static,
{
    pub fn new(rpc: Arc<R>, storage: Arc<S>) -> Self {
        PullBlockRpcHandler { rpc, storage }
    }
}

#[async_trait]
impl<R: Rpc + 'static, S: Storage + 'static> MessageHandler for PullBlockRpcHandler<R, S> {
    type Message = FixedHeight;

    async fn process(&self, ctx: Context, msg: FixedHeight) -> ProtocolResult<()> {
        debug!("message: get rpc pull block {:?}, {:?}", msg.inner, ctx);
        let id = msg.inner;
        let block = self.storage.get_block_by_height(id).await?;

        self.rpc
            .response(
                ctx,
                RPC_RESP_SYNC_PULL_BLOCK,
                FixedBlock::new(block),
                Priority::High,
            )
            .await
    }
}

#[derive(Debug)]
pub struct PullTxsRpcHandler<R, S> {
    rpc:     Arc<R>,
    storage: Arc<S>,
}

impl<R, S> PullTxsRpcHandler<R, S>
where
    R: Rpc + 'static,
    S: Storage + 'static,
{
    pub fn new(rpc: Arc<R>, storage: Arc<S>) -> Self {
        PullTxsRpcHandler { rpc, storage }
    }
}

#[async_trait]
impl<R: Rpc + 'static, S: Storage + 'static> MessageHandler for PullTxsRpcHandler<R, S> {
    type Message = PullTxsRequest;

    async fn process(&self, ctx: Context, msg: PullTxsRequest) -> ProtocolResult<()> {
        debug!("message: get rpc pull txs {:?}", msg.inner.len());
        let mut res = Vec::new();
        for tx in msg.inner.into_iter() {
            res.push(self.storage.get_transaction_by_hash(tx).await?);
        }

        self.rpc
            .response(
                ctx,
                RPC_RESP_SYNC_PULL_TXS,
                FixedSignedTxs::new(res),
                Priority::High,
            )
            .await
    }
}
