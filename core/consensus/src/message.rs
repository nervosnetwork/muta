use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use overlord::types::{AggregatedVote, SignedProposal, SignedVote};
use overlord::Codec;
use rlp::encode;
use serde::{Deserialize, Serialize};

use protocol::traits::{Consensus, ConsensusAdapter, Context, MessageHandler};
use protocol::ProtocolResult;

pub const END_GOSSIP_SIGNED_PROPOSAL: &str = "/gossip/consensus/signed_proposal";
pub const END_GOSSIP_SIGNED_VOTE: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_AGGREGATED_VOTE: &str = "/gossip/consensus/qc";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proposal(pub Vec<u8>);

impl<C: Codec> From<SignedProposal<C>> for Proposal {
    fn from(proposal: SignedProposal<C>) -> Self {
        Proposal(encode(&proposal))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Vote(pub Vec<u8>);

impl From<SignedVote> for Vote {
    fn from(vote: SignedVote) -> Self {
        Vote(encode(&vote))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QC(pub Vec<u8>);

impl From<AggregatedVote> for QC {
    fn from(aggregated_vote: AggregatedVote) -> Self {
        QC(encode(&aggregated_vote))
    }
}

pub struct ProposalMessageHandler<Adapter: ConsensusAdapter, C: Consensus<Adapter>> {
    consensus: Arc<C>,

    pin_a: PhantomData<Adapter>,
}

impl<Adapter: ConsensusAdapter + 'static, C: Consensus<Adapter> + 'static>
    ProposalMessageHandler<Adapter, C>
{
    pub fn new(consensus: Arc<C>) -> Self {
        Self {
            consensus,
            pin_a: PhantomData,
        }
    }
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static, C: Consensus<Adapter> + 'static> MessageHandler
    for ProposalMessageHandler<Adapter, C>
{
    type Message = Proposal;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        self.consensus.set_proposal(ctx, msg.0).await
    }
}

pub struct VoteMessageHandler<Adapter: ConsensusAdapter, C: Consensus<Adapter>> {
    consensus: Arc<C>,

    pin_a: PhantomData<Adapter>,
}

impl<Adapter: ConsensusAdapter + 'static, C: Consensus<Adapter> + 'static>
    VoteMessageHandler<Adapter, C>
{
    pub fn new(consensus: Arc<C>) -> Self {
        Self {
            consensus,
            pin_a: PhantomData,
        }
    }
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static, C: Consensus<Adapter> + 'static> MessageHandler
    for VoteMessageHandler<Adapter, C>
{
    type Message = Vote;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        self.consensus.set_vote(ctx, msg.0).await
    }
}

pub struct QCMessageHandler<Adapter: ConsensusAdapter, C: Consensus<Adapter>> {
    consensus: Arc<C>,

    pin_a: PhantomData<Adapter>,
}

impl<Adapter: ConsensusAdapter + 'static, C: Consensus<Adapter> + 'static>
    QCMessageHandler<Adapter, C>
{
    pub fn new(consensus: Arc<C>) -> Self {
        Self {
            consensus,
            pin_a: PhantomData,
        }
    }
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static, C: Consensus<Adapter> + 'static> MessageHandler
    for QCMessageHandler<Adapter, C>
{
    type Message = QC;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        self.consensus.set_qc(ctx, msg.0).await
    }
}
