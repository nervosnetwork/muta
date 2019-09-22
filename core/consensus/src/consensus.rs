use std::sync::Arc;

use async_trait::async_trait;
use creep::Context;
use overlord::types::{AggregatedVote, Node, OverlordMsg, SignedProposal, SignedVote, Status};
use overlord::{Overlord, OverlordHandler};
use rlp::decode;

use common_crypto::{PrivateKey, Secp256k1PrivateKey};

use protocol::traits::{Consensus, ConsensusAdapter};
use protocol::types::{Epoch, Hash, Proof, SignedTransaction, UserAddress, Validator};
use protocol::ProtocolResult;

use crate::engine::ConsensusEngine;
use crate::fixed_types::{FixedPill, FixedSignedTxs};
use crate::util::OverlordCrypto;
use crate::{ConsensusError, MsgType};

/// Provide consensus
pub struct OverlordConsensus<Adapter: ConsensusAdapter + 'static> {
    /// Overlord consensus protocol instance.
    inner: Arc<Overlord<FixedPill, FixedSignedTxs, ConsensusEngine<Adapter>, OverlordCrypto>>,
    /// An overlord consensus protocol handler.
    handler: OverlordHandler<FixedPill>,
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Consensus for OverlordConsensus<Adapter> {
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()> {
        let signed_proposal: SignedProposal<FixedPill> =
            decode(&proposal).map_err(|_| ConsensusError::DecodeErr(MsgType::SignedProposal))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedProposal(signed_proposal))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn set_vote(&self, ctx: Context, vote: Vec<u8>) -> ProtocolResult<()> {
        let signed_vote: SignedVote =
            decode(&vote).map_err(|_| ConsensusError::DecodeErr(MsgType::SignedVote))?;
        self.handler
            .send_msg(ctx, OverlordMsg::SignedVote(signed_vote))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn set_qc(&self, ctx: Context, qc: Vec<u8>) -> ProtocolResult<()> {
        let aggregated_vote: AggregatedVote =
            decode(&qc).map_err(|_| ConsensusError::DecodeErr(MsgType::AggregateVote))?;
        self.handler
            .send_msg(ctx, OverlordMsg::AggregatedVote(aggregated_vote))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

    async fn update_epoch(
        &self,
        _ctx: Context,
        _epoch: Epoch,
        _signed_txs: Vec<SignedTransaction>,
        _proof: Proof,
    ) -> ProtocolResult<()> {
        Ok(())
    }
}

impl<Adapter: ConsensusAdapter + 'static> OverlordConsensus<Adapter> {
    pub fn new(
        init_epoch_id: u64,
        chain_id: Hash,
        address: UserAddress,
        cycle_limit: u64,
        validators: Vec<Validator>,
        priv_key: Secp256k1PrivateKey,
        adapter: Arc<Adapter>,
    ) -> Self {
        let engine = ConsensusEngine::new(
            chain_id,
            address.clone(),
            cycle_limit,
            validators.clone(),
            Arc::clone(&adapter),
        );

        let crypto = OverlordCrypto::new(priv_key.pub_key(), priv_key);

        let overlord = Overlord::new(address.as_bytes(), engine, crypto);
        let overlord_handler = overlord.get_handler();
        overlord_handler
            .send_msg(
                Context::new(),
                OverlordMsg::RichStatus(handle_genesis(init_epoch_id, validators)),
            )
            .unwrap();

        Self {
            inner:   Arc::new(overlord),
            handler: overlord_handler,
        }
    }

    pub async fn run(&self, interval: u64) -> ProtocolResult<()> {
        self.inner
            .run(interval)
            .await
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;

        Ok(())
    }
}

fn handle_genesis(epoch_id: u64, validators: Vec<Validator>) -> Status {
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
        epoch_id: epoch_id + 1,
        interval: None,
        authority_list,
    }
}
