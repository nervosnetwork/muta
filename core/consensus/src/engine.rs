use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{error::Error, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use futures::lock::Mutex;
use overlord::types::{Commit, Node, OverlordMsg, Status};
use overlord::Consensus as Engine;
use parking_lot::RwLock;
use rlp::Encodable;

use protocol::codec::ProtocolCodec;
use protocol::traits::{
    ConsensusAdapter, Context, CurrentConsensusStatus, MessageTarget, NodeInfo,
};
use protocol::types::{Epoch, EpochHeader, Hash, Pill, Proof, UserAddress, Validator};
use protocol::ProtocolError;

use crate::fixed_types::{FixedPill, FixedSignedTxs};
use crate::message::{
    END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE,
};
use crate::ConsensusError;

/// validator is for create new epoch, and authority is for build overlord
/// status.
pub struct ConsensusEngine<Adapter> {
    current_consensus_status: Arc<RwLock<CurrentConsensusStatus>>,
    node_info:                NodeInfo,
    exemption_hash:           RwLock<HashSet<Bytes>>,

    adapter: Arc<Adapter>,
    lock:    Mutex<()>,
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Engine<FixedPill, FixedSignedTxs>
    for ConsensusEngine<Adapter>
{
    async fn get_epoch(
        &self,
        ctx: Context,
        epoch_id: u64,
    ) -> Result<(FixedPill, Bytes), Box<dyn Error + Send>> {
        let current_consensus_status = { self.current_consensus_status.read().clone() };

        let (ordered_tx_hashes, propose_hashes) = self
            .adapter
            .get_txs_from_mempool(ctx, epoch_id, current_consensus_status.cycles_limit)
            .await?
            .clap();

        if current_consensus_status.epoch_id != epoch_id {
            return Err(ProtocolError::from(ConsensusError::MissingEpochHeader(epoch_id)).into());
        }

        let header = EpochHeader {
            chain_id: self.node_info.chain_id.clone(),
            pre_hash: current_consensus_status.prev_hash,
            epoch_id,
            timestamp: time_now(),
            logs_bloom: current_consensus_status.logs_bloom,
            order_root: current_consensus_status.order_root.clone(),
            confirm_root: current_consensus_status.confirm_root.clone(),
            state_root: current_consensus_status.state_root.clone(),
            receipt_root: current_consensus_status.receipt_root.clone(),
            cycles_used: current_consensus_status.cycles_used,
            proposer: self.node_info.self_address.clone(),
            proof: current_consensus_status.proof.clone(),
            validator_version: 0u64,
            validators: current_consensus_status.validators.clone(),
        };
        let epoch = Epoch {
            header,
            ordered_tx_hashes,
        };

        let fixed_pill = FixedPill::from(Pill {
            epoch,
            propose_hashes,
        });
        let hash = Hash::digest(Bytes::from(fixed_pill.rlp_bytes())).as_bytes();

        {
            let mut set = self.exemption_hash.write();
            set.insert(hash.clone());
        }

        Ok((fixed_pill, hash))
    }

    async fn check_epoch(
        &self,
        ctx: Context,
        _epoch_id: u64,
        hash: Bytes,
        epoch: FixedPill,
    ) -> Result<FixedSignedTxs, Box<dyn Error + Send>> {
        let order_hashes = epoch.get_ordered_hashes();
        let exemption = {
            let set = self.exemption_hash.read();
            set.contains(&hash)
        };

        // If the epoch is proposed by self, it does not need to check. Get full signed
        // transactions directly.
        if !exemption {
            self.adapter
                .sync_txs(ctx.clone(), epoch.get_propose_hashes())
                .await?;
            self.adapter
                .check_txs(ctx.clone(), order_hashes.clone())
                .await?;
        }

        let inner = self.adapter.get_full_txs(ctx, order_hashes).await?;
        Ok(FixedSignedTxs { inner })
    }

    /// **TODO:** the overlord interface and process needs to be changed.
    /// Get the `FixedSignedTxs` from the argument rather than get it from
    /// mempool.
    async fn commit(
        &self,
        ctx: Context,
        epoch_id: u64,
        commit: Commit<FixedPill>,
    ) -> Result<Status, Box<dyn Error + Send>> {
        let lock = self.lock.try_lock();
        if lock.is_none() {
            return Err(
                ProtocolError::from(ConsensusError::Other("lock in sync".to_string())).into(),
            );
        }

        let pill = commit.content.inner;

        // Sorage save the lastest proof.
        let proof = Proof {
            epoch_id:   commit.proof.epoch_id,
            round:      commit.proof.round,
            epoch_hash: Hash::from_bytes(commit.proof.epoch_hash)?,
            signature:  commit.proof.signature.signature,
            bitmap:     commit.proof.signature.address_bitmap,
        };

        self.adapter.save_proof(ctx.clone(), proof.clone()).await?;

        // Get full transactions from mempool temporarily.
        // Storage save the signed transactions.
        let full_txs = self
            .adapter
            .get_full_txs(ctx.clone(), pill.epoch.ordered_tx_hashes.clone())
            .await?;

        self.adapter
            .save_signed_txs(ctx.clone(), full_txs.clone())
            .await?;

        // Storage save the epoch.
        let mut epoch = pill.epoch;
        self.adapter.save_epoch(ctx.clone(), epoch.clone()).await?;

        self.adapter
            .flush_mempool(ctx.clone(), epoch.ordered_tx_hashes.clone())
            .await?;

        let prev_hash = Hash::digest(epoch.encode().await?);

        // TODO: update current consensus status
        let current_consensus_status = {
            let mut current_consensus_status = self.current_consensus_status.write();
            current_consensus_status.epoch_id = epoch_id + 1;
            current_consensus_status.prev_hash = prev_hash;
            current_consensus_status.proof = proof;

            current_consensus_status.clone()
        };

        let status = Status {
            epoch_id:       epoch_id + 1,
            interval:       Some(current_consensus_status.consensus_interval),
            authority_list: covert_to_overlord_authority(&current_consensus_status.validators),
        };
        Ok(status)
    }

    /// Only signed proposal and aggregated vote will be broadcast to others.
    async fn broadcast_to_other(
        &self,
        ctx: Context,
        msg: OverlordMsg<FixedPill>,
    ) -> Result<(), Box<dyn Error + Send>> {
        let (end, msg) = match msg {
            OverlordMsg::SignedProposal(sp) => {
                let bytes = sp.rlp_bytes();
                (END_GOSSIP_SIGNED_PROPOSAL, bytes)
            }

            OverlordMsg::AggregatedVote(av) => {
                let bytes = av.rlp_bytes();
                (END_GOSSIP_AGGREGATED_VOTE, bytes)
            }
            _ => unreachable!(),
        };

        self.adapter
            .transmit(ctx, msg, end, MessageTarget::Broadcast)
            .await?;
        Ok(())
    }

    /// Only signed vote will be transmit to the relayer.
    async fn transmit_to_relayer(
        &self,
        ctx: Context,
        addr: Bytes,
        msg: OverlordMsg<FixedPill>,
    ) -> Result<(), Box<dyn Error + Send>> {
        let msg = match msg {
            OverlordMsg::SignedVote(sv) => sv.rlp_bytes(),
            _ => unreachable!(),
        };

        self.adapter
            .transmit(
                ctx,
                msg,
                END_GOSSIP_SIGNED_VOTE,
                MessageTarget::Specified(UserAddress::from_bytes(addr)?),
            )
            .await?;
        Ok(())
    }

    /// This function is rarely used, so get the authority list from the
    /// RocksDB.
    async fn get_authority_list(
        &self,
        ctx: Context,
        epoch_id: u64,
    ) -> Result<Vec<Node>, Box<dyn Error + Send>> {
        let validators = self.adapter.get_last_validators(ctx, epoch_id).await?;
        let mut res = validators
            .into_iter()
            .map(|v| Node {
                address:        v.address.as_bytes(),
                propose_weight: v.propose_weight,
                vote_weight:    v.vote_weight,
            })
            .collect::<Vec<_>>();

        res.sort();
        Ok(res)
    }
}

impl<Adapter: ConsensusAdapter + 'static> ConsensusEngine<Adapter> {
    pub fn new(
        current_consensus_status: Arc<RwLock<CurrentConsensusStatus>>,
        node_info: NodeInfo,
        adapter: Arc<Adapter>,
    ) -> Self {
        Self {
            current_consensus_status,
            node_info,
            exemption_hash: RwLock::new(HashSet::new()),
            adapter,
            lock: Mutex::new(()),
        }
    }
}

fn covert_to_overlord_authority(validators: &[Validator]) -> Vec<Node> {
    let mut authority = validators
        .iter()
        .map(|v| Node {
            address:        v.address.as_bytes(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect::<Vec<_>>();
    authority.sort();
    authority
}

fn time_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
