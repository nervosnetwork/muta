use async_trait::async_trait;
use derive_more::Constructor;
use futures::channel::mpsc::UnboundedSender;

use core_consensus::message::{Choke, Proposal, Vote, QC};
use core_mempool::{MsgNewTxs, MsgPullTxs};
use protocol::traits::{Context, MessageHandler, TrustFeedback};

use crate::behaviors::Request;

#[derive(Constructor)]
pub struct NewTxsHandler {
    to_commander: UnboundedSender<(Context, Request)>,
}

#[async_trait]
impl MessageHandler for NewTxsHandler {
    type Message = MsgNewTxs;

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        self.to_commander
            .unbounded_send((ctx, Request::NewTx(msg)))
            .unwrap();

        TrustFeedback::Neutral
    }
}

#[derive(Constructor)]
pub struct PullTxsHandler {
    to_commander: UnboundedSender<(Context, Request)>,
}

#[async_trait]
impl MessageHandler for PullTxsHandler {
    type Message = MsgPullTxs;

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        self.to_commander
            .unbounded_send((ctx, Request::PullTxs(msg)))
            .unwrap();

        TrustFeedback::Neutral
    }
}

#[derive(Constructor)]
pub struct ProposalMessageHandler {
    to_commander: UnboundedSender<(Context, Request)>,
}

#[async_trait]
impl MessageHandler for ProposalMessageHandler {
    type Message = Proposal;

    #[muta_apm::derive::tracing_span(name = "handle_proposal", kind = "consensus.message")]
    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        self.to_commander
            .unbounded_send((ctx, Request::RecvProposal(msg)))
            .unwrap();

        TrustFeedback::Good
    }
}

#[derive(Constructor)]
pub struct VoteMessageHandler {
    to_commander: UnboundedSender<(Context, Request)>,
}

#[async_trait]
impl MessageHandler for VoteMessageHandler {
    type Message = Vote;

    #[muta_apm::derive::tracing_span(name = "handle_vote", kind = "consensus.message")]
    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        self.to_commander
            .unbounded_send((ctx, Request::RecvVote(msg)))
            .unwrap();

        TrustFeedback::Good
    }
}

#[derive(Constructor)]
pub struct QCMessageHandler {
    to_commander: UnboundedSender<(Context, Request)>,
}

#[async_trait]
impl MessageHandler for QCMessageHandler {
    type Message = QC;

    #[muta_apm::derive::tracing_span(name = "handle_vote", kind = "consensus.message")]
    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        self.to_commander
            .unbounded_send((ctx, Request::RecvQC(msg)))
            .unwrap();

        TrustFeedback::Good
    }
}

#[derive(Constructor)]
pub struct ChokeMessageHandler {
    to_commander: UnboundedSender<(Context, Request)>,
}

#[async_trait]
impl MessageHandler for ChokeMessageHandler {
    type Message = Choke;

    #[muta_apm::derive::tracing_span(name = "handle_vote", kind = "consensus.message")]
    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        self.to_commander
            .unbounded_send((ctx, Request::RecvChoke(msg)))
            .unwrap();

        TrustFeedback::Good
    }
}

#[derive(Constructor)]
pub struct RemoteHeightMessageHandler {
    to_commander: UnboundedSender<(Context, Request)>,
}

#[async_trait]
impl MessageHandler for RemoteHeightMessageHandler {
    type Message = u64;

    #[muta_apm::derive::tracing_span(name = "handle_vote", kind = "consensus.message")]
    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        self.to_commander
            .unbounded_send((ctx, Request::RecvHeight(msg)))
            .unwrap();

        TrustFeedback::Good
    }
}
