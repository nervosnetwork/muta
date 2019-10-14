use std::sync::Arc;

use async_trait::async_trait;
use bincode::serialize;
use log::debug;
use overlord::types::{AggregatedVote, SignedProposal, SignedVote};
use overlord::Codec;
use rlp::Encodable;
use serde::{Deserialize, Serialize};

use protocol::traits::{Consensus, Context, MessageHandler, Priority, Rpc, Storage};
use protocol::ProtocolResult;

use crate::fixed_types::{
    ConsensusRpcRequest, ConsensusRpcResponse, FixedEpoch, FixedEpochID, FixedSignedTxs,
};

pub const END_GOSSIP_SIGNED_PROPOSAL: &str = "/gossip/consensus/signed_proposal";
pub const END_GOSSIP_SIGNED_VOTE: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_AGGREGATED_VOTE: &str = "/gossip/consensus/qc";
pub const END_GOSSIP_RICH_EPOCH_ID: &str = "/gossip/consensus/rich_epoch_id";
pub const RPC_SYNC_PULL: &str = "/rpc_call/consensus/sync_pull";
pub const RPC_RESP_SYNC_PULL: &str = "/rpc_resp/consensus/sync_pull";

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
pub struct RichEpochID(pub Vec<u8>);

impl From<FixedEpochID> for RichEpochID {
    fn from(id: FixedEpochID) -> Self {
        RichEpochID(serialize(&id).unwrap())
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

pub struct RichEpochIDMessageHandler<C> {
    consensus: Arc<C>,
}

impl<C: Consensus + 'static> RichEpochIDMessageHandler<C> {
    pub fn new(consensus: Arc<C>) -> Self {
        Self { consensus }
    }
}

#[async_trait]
impl<C: Consensus + 'static> MessageHandler for RichEpochIDMessageHandler<C> {
    type Message = RichEpochID;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        self.consensus.update_epoch(ctx, msg.0).await
    }
}

#[derive(Debug)]
pub struct RpcHandler<R, S> {
    rpc:     Arc<R>,
    storage: Arc<S>,
}

#[async_trait]
impl<R: Rpc + 'static, S: Storage + 'static> MessageHandler for RpcHandler<R, S> {
    type Message = ConsensusRpcRequest;

    async fn process(&self, ctx: Context, msg: ConsensusRpcRequest) -> ProtocolResult<()> {
        match msg {
            ConsensusRpcRequest::PullEpochs(ep) => {
                debug!("message: get rpc pull epoch {:?}, {:?}", ep, ctx);
                let res = self.storage.get_epoch_by_epoch_id(ep).await?;

                self.rpc
                    .response(
                        ctx,
                        RPC_RESP_SYNC_PULL,
                        ConsensusRpcResponse::PullEpochs(Box::new(FixedEpoch::new(res))),
                        Priority::High,
                    )
                    .await
            }

            ConsensusRpcRequest::PullTxs(txs) => {
                let mut res = Vec::new();
                for tx in txs.inner.into_iter() {
                    res.push(self.storage.get_transaction_by_hash(tx).await?);
                }

                self.rpc
                    .response(
                        ctx,
                        RPC_RESP_SYNC_PULL,
                        ConsensusRpcResponse::PullTxs(Box::new(FixedSignedTxs::new(res))),
                        Priority::High,
                    )
                    .await
            }
        }
    }
}

impl<R, S> RpcHandler<R, S>
where
    R: Rpc + 'static,
    S: Storage + 'static,
{
    pub fn new(rpc: Arc<R>, storage: Arc<S>) -> Self {
        RpcHandler { rpc, storage }
    }
}
