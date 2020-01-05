use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use bincode::deserialize;
use creep::Context;
use futures::channel::mpsc::UnboundedSender;
use overlord::types::{AggregatedVote, Node, OverlordMsg, SignedProposal, SignedVote, Status};
use overlord::{DurationConfig, Overlord, OverlordHandler};

use common_crypto::{BlsCommonReference, BlsPrivateKey, BlsPublicKey};

use protocol::traits::{Consensus, ConsensusAdapter, NodeInfo};
use protocol::types::Validator;
use protocol::{Bytes, ProtocolResult};

use crate::engine::ConsensusEngine;
use crate::fixed_types::{FixedEpochID, FixedPill, FixedSignedTxs};
use crate::status::StatusAgent;
use crate::synchronization::Synchronization;
use crate::util::OverlordCrypto;
use crate::{ConsensusError, MsgType};

/// Provide consensus
pub struct OverlordConsensus<Adapter: ConsensusAdapter + 'static> {
    /// Overlord consensus protocol instance.
    inner:   Arc<Overlord<FixedPill, FixedSignedTxs, ConsensusEngine<Adapter>, OverlordCrypto>>,
    /// An overlord consensus protocol handler.
    handler: OverlordHandler<FixedPill>,
    /// A synchronization module.
    sync:    UnboundedSender<(u64, Context)>,
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Consensus for OverlordConsensus<Adapter> {
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()> {
        let signed_proposal: SignedProposal<FixedPill> = rlp::decode(&proposal)
            .map_err(|_| ConsensusError::DecodeErr(MsgType::SignedProposal))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedProposal(signed_proposal))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn set_vote(&self, ctx: Context, vote: Vec<u8>) -> ProtocolResult<()> {
        let signed_vote: SignedVote =
            rlp::decode(&vote).map_err(|_| ConsensusError::DecodeErr(MsgType::SignedVote))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedVote(signed_vote))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn set_qc(&self, ctx: Context, qc: Vec<u8>) -> ProtocolResult<()> {
        let aggregated_vote: AggregatedVote =
            rlp::decode(&qc).map_err(|_| ConsensusError::DecodeErr(MsgType::AggregateVote))?;
        self.handler
            .send_msg(ctx, OverlordMsg::AggregatedVote(aggregated_vote))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn update_epoch(&self, ctx: Context, msg: Vec<u8>) -> ProtocolResult<()> {
        let epoch_id: FixedEpochID =
            deserialize(&msg).map_err(|_| ConsensusError::DecodeErr(MsgType::RichEpochID))?;
        self.sync
            .unbounded_send((epoch_id.inner, ctx))
            .map_err(|_| ConsensusError::Other("Send sync msg error".to_string()))?;
        Ok(())
    }
}

impl<Adapter: ConsensusAdapter + 'static> OverlordConsensus<Adapter> {
    pub fn new(
        status_agent: StatusAgent,
        node_info: NodeInfo,
        addr_pubkey: HashMap<Bytes, BlsPublicKey>,
        priv_key: BlsPrivateKey,
        common_ref: BlsCommonReference,
        adapter: Arc<Adapter>,
    ) -> (Self, Synchronization<Adapter>) {
        let engine = Arc::new(ConsensusEngine::new(
            status_agent.clone(),
            node_info.clone(),
            Arc::clone(&adapter),
        ));

        let crypto = OverlordCrypto::new(priv_key, addr_pubkey, common_ref);
        let overlord = Overlord::new(
            node_info.self_address.as_bytes(),
            Arc::clone(&engine),
            crypto,
        );
        let overlord_handler = overlord.get_handler();

        let status = status_agent.to_inner();
        overlord_handler
            .send_msg(
                Context::new(),
                OverlordMsg::RichStatus(gen_overlord_status(
                    status.epoch_id,
                    status.consensus_interval,
                    status.validators,
                )),
            )
            .unwrap();

        let (sync, rx) = Synchronization::new(overlord_handler.clone(), Arc::clone(&engine));
        let consensus = OverlordConsensus {
            inner:   Arc::new(overlord),
            handler: overlord_handler,
            sync:    rx,
        };

        (consensus, sync)
    }

    pub async fn run(
        &self,
        interval: u64,
        timer_config: Option<DurationConfig>,
    ) -> ProtocolResult<()> {
        self.inner
            .run(interval, timer_config)
            .await
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;

        Ok(())
    }
}

fn gen_overlord_status(epoch_id: u64, interval: u64, validators: Vec<Validator>) -> Status {
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
        epoch_id,
        interval: Some(interval),
        authority_list,
    }
}
