use std::sync::Arc;

use async_trait::async_trait;
use creep::Context;
use futures::lock::Mutex;
use overlord::types::{
    AggregatedVote, Node, OverlordMsg, SignedChoke, SignedProposal, SignedVote, Status,
};
use overlord::{DurationConfig, Overlord, OverlordHandler};

use common_apm::muta_apm;

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
        Overlord<FixedPill, ConsensusEngine<Adapter>, OverlordCrypto, ConsensusEngine<Adapter>>,
    >,
    /// An overlord consensus protocol handler.
    handler: OverlordHandler<FixedPill>,
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Consensus for OverlordConsensus<Adapter> {
    #[muta_apm::derive::tracing_span(kind = "consensus")]
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()> {
        let signed_proposal: SignedProposal<FixedPill> = rlp::decode(&proposal)
            .map_err(|_| ConsensusError::DecodeErr(ConsensusType::SignedProposal))?;

        let msg = OverlordMsg::SignedProposal(signed_proposal);
        tracing_overlord_message(ctx.clone(), &msg);

        self.handler
            .send_msg(ctx, msg)
            .expect("Overlord handler disconnect");
        Ok(())
    }

    async fn set_vote(&self, ctx: Context, vote: Vec<u8>) -> ProtocolResult<()> {
        let ctx = match muta_apm::MUTA_TRACER.span("consensus.set_vote", vec![
            muta_apm::rustracing::tag::Tag::new("kind", "consensus"),
        ]) {
            Some(mut span) => {
                span.log(|log| {
                    log.time(std::time::SystemTime::now());
                });
                ctx.with_value("parent_span_ctx", span.context().cloned())
            }
            None => ctx,
        };

        let signed_vote: SignedVote =
            rlp::decode(&vote).map_err(|_| ConsensusError::DecodeErr(ConsensusType::SignedVote))?;

        let msg = OverlordMsg::SignedVote(signed_vote);
        tracing_overlord_message(ctx.clone(), &msg);

        self.handler
            .send_msg(ctx, msg)
            .expect("Overlord handler disconnect");
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "consensus")]
    async fn set_qc(&self, ctx: Context, qc: Vec<u8>) -> ProtocolResult<()> {
        let aggregated_vote: AggregatedVote = rlp::decode(&qc)
            .map_err(|_| ConsensusError::DecodeErr(ConsensusType::AggregateVote))?;

        let msg = OverlordMsg::AggregatedVote(aggregated_vote);
        tracing_overlord_message(ctx.clone(), &msg);

        self.handler
            .send_msg(ctx, msg)
            .expect("Overlord handler disconnect");
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "consensus")]
    async fn set_choke(&self, ctx: Context, choke: Vec<u8>) -> ProtocolResult<()> {
        let signed_choke: SignedChoke = rlp::decode(&choke)
            .map_err(|_| ConsensusError::DecodeErr(ConsensusType::SignedChoke))?;

        let msg = OverlordMsg::SignedChoke(signed_choke);
        tracing_overlord_message(ctx.clone(), &msg);

        self.handler
            .send_msg(ctx, msg)
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

        let overlord = Overlord::new(node_info.self_pub_key, Arc::clone(&engine), crypto, engine);
        let overlord_handler = overlord.get_handler();
        let status = status_agent.to_inner();

        if status.latest_committed_height == 0 {
            overlord_handler
                .send_msg(
                    Context::new(),
                    OverlordMsg::RichStatus(gen_overlord_status(
                        status.latest_committed_height + 1,
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
        init_height: u64,
        interval: u64,
        authority_list: Vec<Node>,
        timer_config: Option<DurationConfig>,
    ) -> ProtocolResult<()> {
        self.inner
            .run(init_height, interval, authority_list, timer_config)
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
            address:        v.pub_key.clone(),
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

trait OverlordMsgExt {
    fn get_height(&self) -> String;
    fn get_round(&self) -> String;
}

impl<T: overlord::Codec> OverlordMsgExt for OverlordMsg<T> {
    fn get_height(&self) -> String {
        match self {
            OverlordMsg::SignedProposal(sp) => sp.proposal.height.to_string(),
            OverlordMsg::SignedVote(sv) => sv.get_height().to_string(),
            OverlordMsg::AggregatedVote(av) => av.get_height().to_string(),
            OverlordMsg::RichStatus(s) => s.height.to_string(),
            OverlordMsg::SignedChoke(sc) => sc.choke.height.to_string(),
            _ => "".to_owned(),
        }
    }

    fn get_round(&self) -> String {
        match self {
            OverlordMsg::SignedProposal(sp) => sp.proposal.round.to_string(),
            OverlordMsg::SignedVote(sv) => sv.get_round().to_string(),
            OverlordMsg::AggregatedVote(av) => av.get_round().to_string(),
            OverlordMsg::SignedChoke(sc) => sc.choke.round.to_string(),
            _ => "".to_owned(),
        }
    }
}

#[muta_apm::derive::tracing_span(
    kind = "consensus",
    logs = "{
    'height': 'msg.get_height()',
    'round': 'msg.get_round()'
}"
)]
pub fn tracing_overlord_message<T: overlord::Codec>(ctx: Context, msg: &OverlordMsg<T>) {
    let _ = msg;
}
