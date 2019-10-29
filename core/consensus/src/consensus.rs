use std::sync::Arc;

use async_trait::async_trait;
use bincode::deserialize;
use creep::Context;
use futures::lock::Mutex;
use log::{debug, info};
use overlord::types::{AggregatedVote, Node, OverlordMsg, SignedProposal, SignedVote, Status};
use overlord::{DurationConfig, Overlord, OverlordHandler};
use parking_lot::RwLock;

use common_crypto::{PrivateKey, Secp256k1PrivateKey};

use protocol::traits::{Consensus, ConsensusAdapter, CurrentConsensusStatus, NodeInfo};
use protocol::types::{Address, Hash, Proof, Validator};
use protocol::{fixed_codec::ProtocolFixedCodec, ProtocolResult};

use crate::engine::ConsensusEngine;
use crate::fixed_types::{FixedEpochID, FixedPill, FixedSignedTxs};
use crate::util::OverlordCrypto;
use crate::{ConsensusError, MsgType};

/// Provide consensus
#[allow(dead_code)]
pub struct OverlordConsensus<Adapter: ConsensusAdapter + 'static> {
    /// Overlord consensus protocol instance.
    inner: Arc<Overlord<FixedPill, FixedSignedTxs, ConsensusEngine<Adapter>, OverlordCrypto>>,
    /// An overlord consensus protocol handler.
    handler: OverlordHandler<FixedPill>,
    /// A consensus engine for synchronous.
    engine: Arc<ConsensusEngine<Adapter>>,
    /// Synchronization lock.
    lock: Mutex<()>,
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
        let sync_lock = self.lock.try_lock();
        if sync_lock.is_none() {
            // Synchronization is processing.
            return Ok(());
        }

        // Reveive the rich epoch ID.
        let epoch_id: FixedEpochID =
            deserialize(&msg).map_err(|_| ConsensusError::DecodeErr(MsgType::RichEpochID))?;
        let rich_epoch_id = epoch_id.inner - 1;

        // TODO: fix to get_epoch_by_epoch_id()
        let current_epoch_id = self
            .engine
            .get_current_epoch_id(ctx.clone())
            .await
            .unwrap_or(1u64);

        if current_epoch_id >= rich_epoch_id - 1 {
            return Ok(());
        }

        // Lock the consensus engine, block commit process.
        let commit_lock = self.engine.lock.try_lock();
        if commit_lock.is_none() {
            return Ok(());
        }

        info!("self {}, chain {}", current_epoch_id, rich_epoch_id);
        info!("consensus: start synchronization");

        let mut state_root = Hash::from_empty();
        let mut current_hash = if current_epoch_id != 0 {
            let current_epoch = self
                .engine
                .get_epoch_by_id(ctx.clone(), current_epoch_id)
                .await?;
            state_root = current_epoch.header.state_root.clone();
            let tmp = Hash::digest(current_epoch.encode_fixed()?);

            // Check epoch for the first time.
            let epoch_header = self
                .engine
                .pull_epoch(ctx.clone(), current_epoch_id + 1)
                .await?
                .header;
            self.check_proof(current_epoch_id + 1, epoch_header.proof.clone())?;
            if tmp != epoch_header.pre_hash {
                return Err(ConsensusError::SyncEpochHashErr(current_epoch_id + 1).into());
            }
            tmp
        } else {
            Hash::from_empty()
        };

        // Start to synchronization.
        for id in (current_epoch_id + 1)..=rich_epoch_id {
            info!("consensus: start synchronization epoch {}", id);

            // First pull a new block.
            debug!("consensus: synchronization pull epoch {}", id);
            let epoch = self.engine.pull_epoch(ctx.clone(), id).await?;

            // Check proof and previous hash.
            debug!("consensus: synchronization check proof and previous hash");
            let proof = epoch.header.proof.clone();
            self.check_proof(id, proof.clone())?;
            if id != 1 && current_hash != epoch.header.pre_hash {
                return Err(ConsensusError::SyncEpochHashErr(id).into());
            }
            self.engine.save_proof(ctx.clone(), proof.clone()).await?;

            // Then pull signed transactions.
            debug!("consensus: synchronization pull signed transactions");
            let txs = self
                .engine
                .pull_txs(ctx.clone(), epoch.ordered_tx_hashes.clone())
                .await?;

            // After get the signed transactions:
            // 1. Execute the signed transactions.
            // 2. Save the signed transactions.
            // 3. Save the latest proof.
            // 4. Save the new epoch.
            // 5. Save the receipt.
            debug!("consensus: synchronization executor the epoch");
            let exec_resp = self
                .engine
                .exec(
                    state_root.clone(),
                    epoch.header.epoch_id,
                    Address::User(epoch.header.proposer.clone()),
                    txs.clone(),
                )
                .await?;
            state_root = exec_resp.state_root.clone();

            debug!("consensus: synchronization update the rich status");
            self.engine
                .update_status(epoch.header.epoch_id, epoch.clone(), proof, exec_resp, txs)
                .await?;

            // Update the previous hash and last epoch.
            info!("consensus: finish synchronization {} epoch", id);
            current_hash = Hash::digest(epoch.encode_fixed()?);
        }

        debug!(
            "consensus: synchronization send overlord rich status {}",
            rich_epoch_id
        );
        let status = Status {
            epoch_id:       rich_epoch_id + 1,
            interval:       Some(self.engine.get_current_interval()),
            authority_list: self.engine.get_current_authority_list(),
        };

        self.handler
            .send_msg(ctx, OverlordMsg::RichStatus(status))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }
}

impl<Adapter: ConsensusAdapter + 'static> OverlordConsensus<Adapter> {
    pub fn new(
        current_consensus_status: CurrentConsensusStatus,
        node_info: NodeInfo,
        priv_key: Secp256k1PrivateKey,
        adapter: Arc<Adapter>,
    ) -> Self {
        let current_consensus_status = Arc::new(RwLock::new(current_consensus_status));

        let engine = Arc::new(ConsensusEngine::new(
            Arc::clone(&current_consensus_status),
            node_info.clone(),
            Arc::clone(&adapter),
        ));

        let crypto = OverlordCrypto::new(priv_key.pub_key(), priv_key);
        let overlord = Overlord::new(
            node_info.self_address.as_bytes(),
            Arc::clone(&engine),
            crypto,
        );
        let overlord_handler = overlord.get_handler();

        overlord_handler
            .send_msg(
                Context::new(),
                OverlordMsg::RichStatus(gen_overlord_status(
                    current_consensus_status.read().epoch_id,
                    current_consensus_status.read().consensus_interval,
                    current_consensus_status.read().validators.clone(),
                )),
            )
            .unwrap();

        Self {
            inner: Arc::new(overlord),
            handler: overlord_handler,
            lock: Mutex::new(()),
            engine,
        }
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

    fn check_proof(&self, _epoch_id: u64, _proof: Proof) -> ProtocolResult<()> {
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
