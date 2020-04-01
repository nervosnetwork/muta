#![allow(clippy::type_complexity)]

use std::sync::Arc;

use async_trait::async_trait;
use creep::Context;
use futures::lock::Mutex;
use overlord::types::{
    AggregatedVote, Node, OverlordMsg, SignedChoke, SignedProposal, SignedVote, Status,
};
use overlord::{DurationConfig, Overlord, OverlordHandler};

use protocol::traits::{Consensus, ConsensusAdapter, NodeInfo};
use protocol::types::Validator;
use protocol::ProtocolResult;

use crate::engine::ConsensusEngine;
use crate::fixed_types::FixedPill;
use crate::status::StatusAgent;
use crate::util::OverlordCrypto;
use crate::wal::SignedTxsWAL;
use crate::{ConsensusError, ConsensusType};

/// Provide consensus
pub struct OverlordConsensus<Adapter: ConsensusAdapter + 'static> {
    /// Overlord consensus protocol instance.
    inner: Arc<
        Overlord<
            FixedPill,
            ConsensusEngine<Adapter>,
            OverlordCrypto,
            ConsensusEngine<Adapter>,
            ConsensusEngine<Adapter>,
        >,
    >,
    /// An overlord consensus protocol handler.
    handler: OverlordHandler<FixedPill>,
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Consensus for OverlordConsensus<Adapter> {
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()> {
        let signed_proposal: SignedProposal<FixedPill> = rlp::decode(&proposal)
            .map_err(|_| ConsensusError::DecodeErr(ConsensusType::SignedProposal))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedProposal(signed_proposal))
            .expect("Overlord handler disconnect");
        Ok(())
    }

    async fn set_vote(&self, ctx: Context, vote: Vec<u8>) -> ProtocolResult<()> {
        let signed_vote: SignedVote =
            rlp::decode(&vote).map_err(|_| ConsensusError::DecodeErr(ConsensusType::SignedVote))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedVote(signed_vote))
            .expect("Overlord handler disconnect");
        Ok(())
    }

    async fn set_qc(&self, ctx: Context, qc: Vec<u8>) -> ProtocolResult<()> {
        let aggregated_vote: AggregatedVote = rlp::decode(&qc)
            .map_err(|_| ConsensusError::DecodeErr(ConsensusType::AggregateVote))?;
        self.handler
            .send_msg(ctx, OverlordMsg::AggregatedVote(aggregated_vote))
            .expect("Overlord handler disconnect");
        Ok(())
    }

    async fn set_choke(&self, ctx: Context, choke: Vec<u8>) -> ProtocolResult<()> {
        let signed_choke: SignedChoke = rlp::decode(&choke)
            .map_err(|_| ConsensusError::DecodeErr(ConsensusType::SignedChoke))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedChoke(signed_choke))
            .expect("Overlord handler disconnect");
        Ok(())
    }
}

impl<Adapter: ConsensusAdapter + 'static> OverlordConsensus<Adapter> {
    pub fn new(
        status_agent: StatusAgent,
        node_info: NodeInfo,
        crypto: Arc<OverlordCrypto>,
        txs_wal: Arc<SignedTxsWAL>,
        adapter: Arc<Adapter>,
        lock: Arc<Mutex<()>>,
    ) -> Self {
        let engine = Arc::new(ConsensusEngine::new(
            status_agent.clone(),
            node_info.clone(),
            txs_wal,
            Arc::clone(&adapter),
            Arc::clone(&crypto),
            lock,
        ));

        let overlord = Overlord::new(
            node_info.self_address.as_bytes(),
            Arc::clone(&engine),
            crypto,
            Arc::clone(&engine),
            engine,
        );
        let overlord_handler = overlord.get_handler();
        let status = status_agent.to_inner();

        if status.current_height == 0 {
            overlord_handler
                .send_msg(
                    Context::new(),
                    OverlordMsg::RichStatus(gen_overlord_status(
                        status.current_height + 1,
                        status.consensus_interval,
                        status.propose_ratio,
                        status.prevote_ratio,
                        status.precommit_ratio,
                        status.brake_ratio,
                        status.validators,
                    )),
                )
                .unwrap();
        }

        Self {
            inner:   Arc::new(overlord),
            handler: overlord_handler,
        }
    }

    pub fn get_overlord_handler(&self) -> OverlordHandler<FixedPill> {
        self.handler.clone()
    }

    pub async fn run(
        &self,
        interval: u64,
        authority_list: Vec<Node>,
        timer_config: Option<DurationConfig>,
    ) -> ProtocolResult<()> {
        self.inner
            .run(interval, authority_list, timer_config)
            .await
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;

        Ok(())
    }
}

pub fn gen_overlord_status(
    height: u64,
    interval: u64,
    propose_ratio: u64,
    prevote_ratio: u64,
    precommit_ratio: u64,
    brake_ratio: u64,
    validators: Vec<Validator>,
) -> Status {
    let mut authority_list = validators
        .into_iter()
        .map(|v| Node {
            address:        v.address.as_bytes(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect::<Vec<_>>();

    authority_list.sort();

    Status {
        height,
        interval: Some(interval),
        timer_config: Some(DurationConfig {
            propose_ratio,
            prevote_ratio,
            precommit_ratio,
            brake_ratio,
        }),
        authority_list,
    }
}
