use std::sync::Arc;

use async_trait::async_trait;
use bincode::serialize;
use futures::{future::try_join_all, TryFutureExt};
use log::warn;
use overlord::types::{AggregatedVote, SignedChoke, SignedProposal, SignedVote};
use overlord::Codec;
use rlp::Encodable;
use serde::{Deserialize, Serialize};

use protocol::traits::{
    Consensus, Context, MessageHandler, Priority, Rpc, Storage, Synchronization, TrustFeedback,
};
use protocol::ProtocolError;

use core_storage::StorageError;

use crate::fixed_types::{FixedBlock, FixedHeight, FixedProof, FixedSignedTxs, PullTxsRequest};

pub const END_GOSSIP_SIGNED_PROPOSAL: &str = "/gossip/consensus/signed_proposal";
pub const END_GOSSIP_SIGNED_VOTE: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_AGGREGATED_VOTE: &str = "/gossip/consensus/qc";
pub const END_GOSSIP_SIGNED_CHOKE: &str = "/gossip/consensus/signed_choke";
pub const RPC_SYNC_PULL_BLOCK: &str = "/rpc_call/consensus/sync_pull_block";
pub const RPC_RESP_SYNC_PULL_BLOCK: &str = "/rpc_resp/consensus/sync_pull_block";
pub const RPC_SYNC_PULL_TXS: &str = "/rpc_call/consensus/sync_pull_txs";
pub const RPC_RESP_SYNC_PULL_TXS: &str = "/rpc_resp/consensus/sync_pull_txs";
pub const BROADCAST_HEIGHT: &str = "/gossip/consensus/broadcast_height";
pub const RPC_SYNC_PULL_PROOF: &str = "/rpc_call/consensus/sync_pull_proof";
pub const RPC_RESP_SYNC_PULL_PROOF: &str = "/rpc_resp/consensus/sync_pull_proof";

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Choke(pub Vec<u8>);

impl From<SignedChoke> for Choke {
    fn from(signed_choke: SignedChoke) -> Self {
        Choke(signed_choke.rlp_bytes())
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

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        if let Err(e) = self.consensus.set_proposal(ctx, msg.0).await {
            warn!("set proposal {}", e);
        }
        // FIXME
        TrustFeedback::Neutral
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

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        if let Err(e) = self.consensus.set_vote(ctx, msg.0).await {
            warn!("set vote {}", e);
        }
        // FIXME
        TrustFeedback::Neutral
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

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        if let Err(e) = self.consensus.set_qc(ctx, msg.0).await {
            warn!("set qc {}", e);
        }
        // FIXME
        TrustFeedback::Neutral
    }
}

pub struct ChokeMessageHandler<C> {
    consensus: Arc<C>,
}

impl<C: Consensus + 'static> ChokeMessageHandler<C> {
    pub fn new(consensus: Arc<C>) -> Self {
        Self { consensus }
    }
}

#[async_trait]
impl<C: Consensus + 'static> MessageHandler for ChokeMessageHandler<C> {
    type Message = Choke;

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        if let Err(e) = self.consensus.set_choke(ctx, msg.0).await {
            warn!("set choke {}", e);
        }
        // FIXME
        TrustFeedback::Neutral
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

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        if let Err(e) = self.synchronization.receive_remote_block(ctx, msg).await {
            warn!("sync: receive remote block {}", e);
            if e.to_string().contains("timeout") {
                return TrustFeedback::Bad("sync block timeout".to_owned());
            }
        }
        // FIXME
        TrustFeedback::Neutral
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

    async fn process(&self, ctx: Context, msg: FixedHeight) -> TrustFeedback {
        let id = msg.inner;
        let ret = self
            .storage
            .get_block_by_height(id)
            .await
            .map(FixedBlock::new);
        self.rpc
            .response(ctx, RPC_RESP_SYNC_PULL_BLOCK, ret, Priority::High)
            .unwrap_or_else(move |e: ProtocolError| warn!("[core_consensus] push block {}", e))
            .await;

        // FIXME
        TrustFeedback::Neutral
    }
}

#[derive(Debug)]
pub struct PullProofRpcHandler<R, S> {
    rpc:     Arc<R>,
    storage: Arc<S>,
}

impl<R, S> PullProofRpcHandler<R, S>
where
    R: Rpc + 'static,
    S: Storage + 'static,
{
    pub fn new(rpc: Arc<R>, storage: Arc<S>) -> Self {
        PullProofRpcHandler { rpc, storage }
    }
}

#[async_trait]
impl<R: Rpc + 'static, S: Storage + 'static> MessageHandler for PullProofRpcHandler<R, S> {
    type Message = FixedHeight;

    async fn process(&self, ctx: Context, height: FixedHeight) {
        let height = height.inner;
        let latest_proof = self.storage.get_latest_proof().await;

        let ret = match latest_proof {
            Ok(latest_proof) => match height {
                height if height < latest_proof.height => {
                    match self.storage.get_block_by_height(height + 1).await {
                        Ok(next_block) => Ok(next_block.header.proof),
                        Err(_) => Err(StorageError::GetNone.into()),
                    }
                }
                height if height == latest_proof.height => Ok(latest_proof),
                _ => Err(StorageError::GetNone.into()),
            },
            Err(_) => Err(StorageError::GetNone.into()),
        };
        self.rpc
            .response(
                ctx,
                RPC_RESP_SYNC_PULL_PROOF,
                ret.map(FixedProof::new),
                Priority::High,
            )
            .unwrap_or_else(move |e: ProtocolError| warn!("[core_consensus] push proof {}", e))
            .await;
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

    async fn process(&self, ctx: Context, msg: PullTxsRequest) -> TrustFeedback {
        let futs = msg
            .inner
            .into_iter()
            .map(|tx_hash| self.storage.get_transaction_by_hash(tx_hash))
            .collect::<Vec<_>>();
        let ret = try_join_all(futs).await.map(FixedSignedTxs::new);

        self.rpc
            .response(ctx, RPC_RESP_SYNC_PULL_TXS, ret, Priority::High)
            .unwrap_or_else(move |e: ProtocolError| warn!("[core_consensus] push txs {}", e))
            .await;

        // FIXME
        TrustFeedback::Neutral
    }
}
