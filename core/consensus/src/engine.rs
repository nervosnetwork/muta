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
use protocol::traits::{ConsensusAdapter, Context, MessageTarget};
use protocol::types::{
    Bloom, Epoch, EpochHeader, Hash, MerkleRoot, Pill, Proof, UserAddress, Validator,
    GENESIS_EPOCH_ID,
};
use protocol::ProtocolError;

use crate::fixed_types::{FixedPill, FixedSignedTxs};
use crate::message::{
    END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE,
};
use crate::ConsensusError;

const INIT_ROUND: u64 = 0;

/// validator is for create new epoch, and authority is for build overlord
/// status.
pub struct ConsensusEngine<Adapter> {
    chain_id:       Hash,
    address:        UserAddress,
    cycle_limit:    u64,
    exemption_hash: RwLock<HashSet<Bytes>>,
    latest_header:  RwLock<HeaderCache>,
    validators:     Vec<Validator>,
    authority:      Vec<Node>,
    adapter:        Arc<Adapter>,

    lock: Mutex<()>,
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
        let tmp = epoch_id;
        let (ordered_tx_hashes, propose_hashes) = self
            .adapter
            .get_txs_from_mempool(ctx, epoch_id, self.cycle_limit)
            .await?
            .clap();

        let cache = {
            let header = self.latest_header.read();

            if header.epoch_id == epoch_id {
                header.clone()
            } else {
                return Err(
                    ProtocolError::from(ConsensusError::MissingEpochHeader(epoch_id)).into(),
                );
            }
        };

        let header = EpochHeader {
            chain_id:          self.chain_id.clone(),
            pre_hash:          cache.prev_hash,
            epoch_id:          tmp,
            timestamp:         time_now(),
            logs_bloom:        cache.logs_bloom,
            order_root:        cache.order_root,
            confirm_root:      cache.confirm_root,
            state_root:        cache.state_root,
            receipt_root:      cache.receipt_root,
            cycles_used:       cache.cycles_used,
            proposer:          self.address.clone(),
            proof:             cache.proof,
            validator_version: 0u64,
            validators:        self.validators.clone(),
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

        let mut set = self.exemption_hash.write();
        set.insert(hash.clone());
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
        let tmp = commit.proof;

        // Sorage save the lastest proof.
        let proof = Proof {
            epoch_id:   tmp.epoch_id,
            round:      tmp.round,
            epoch_hash: Hash::from_bytes(tmp.epoch_hash)?,
            signature:  tmp.signature.signature,
            bitmap:     tmp.signature.address_bitmap,
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

        // TODO: update header cache.
        let mut header = self.latest_header.write();
        header.epoch_id = epoch_id + 1;
        header.prev_hash = prev_hash;
        header.proof = proof;

        let status = Status {
            epoch_id:       header.epoch_id,
            interval:       None,
            authority_list: self.authority.clone(),
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
        chain_id: Hash,
        address: UserAddress,
        cycle_limit: u64,
        validators: Vec<Validator>,
        adapter: Arc<Adapter>,
    ) -> Self {
        let mut authority = validators
            .clone()
            .into_iter()
            .map(|v| Node {
                address:        v.address.as_bytes(),
                propose_weight: v.propose_weight,
                vote_weight:    v.vote_weight,
            })
            .collect::<Vec<_>>();

        authority.sort();

        Self {
            chain_id,
            address,
            cycle_limit,
            exemption_hash: RwLock::new(HashSet::new()),
            latest_header: RwLock::new(HeaderCache::default()),
            validators,
            authority,
            adapter,
            lock: Mutex::new(()),
        }
    }
}

#[derive(Clone, Debug, Hash)]
struct HeaderCache {
    pub epoch_id:     u64,
    pub prev_hash:    Hash,
    pub logs_bloom:   Bloom,
    pub order_root:   MerkleRoot,
    pub confirm_root: Vec<MerkleRoot>,
    pub state_root:   MerkleRoot,
    pub receipt_root: Vec<MerkleRoot>,
    pub cycles_used:  u64,
    pub proof:        Proof,
}

impl Default for HeaderCache {
    fn default() -> Self {
        let genesis_proof = Proof {
            epoch_id:   GENESIS_EPOCH_ID,
            round:      INIT_ROUND,
            epoch_hash: Hash::from_empty(),
            signature:  Bytes::default(),
            bitmap:     Bytes::default(),
        };

        HeaderCache {
            epoch_id:     GENESIS_EPOCH_ID + 1,
            prev_hash:    Hash::from_empty(),
            logs_bloom:   Bloom::zero(),
            order_root:   MerkleRoot::from_empty(),
            confirm_root: Vec::new(),
            state_root:   MerkleRoot::from_empty(),
            receipt_root: Vec::new(),
            cycles_used:  0,
            proof:        genesis_proof,
        }
    }
}

fn time_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
