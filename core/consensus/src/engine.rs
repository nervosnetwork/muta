use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bincode::serialize;
use bytes::Bytes;
use overlord::types::{Commit, Node, OverlordMsg, Status};
use overlord::Consensus as Overlord;
use parking_lot::RwLock;
use rlp::Encodable;

use protocol::traits::{
    ConsensusAdapter, Context, Gossip, MemPool, MemPoolAdapter, MessageTarget, Storage,
    StorageAdapter,
};
use protocol::types::{
    Bloom, Epoch, EpochHeader, Hash, MerkleRoot, Pill, Proof, Receipt, UserAddress,
};
use protocol::ProtocolError;

use crate::adapter::OverlordConsensusAdapter;
use crate::fixed_types::{FixedPill, FixedSignedTxs};
use crate::message::{
    Proposal, Vote, END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE,
    QC,
};
use crate::ConsensusError;

pub struct ConsensusEngine<
    G: Gossip,
    M: MemPool<MA>,
    S: Storage<SA>,
    MA: MemPoolAdapter,
    SA: StorageAdapter,
> {
    chain_id:       Hash,
    address:        UserAddress,
    cycle_limit:    u64,
    exemption_hash: RwLock<HashSet<Bytes>>,
    state_root:     MerkleRoot,
    order_root:     MerkleRoot,
    header_cache:   HashMap<u64, HeaderCache>,
    adapter:        OverlordConsensusAdapter<G, M, S, MA, SA>,
}

#[async_trait]
impl<G, M, S, MA, SA> Overlord<FixedPill, FixedSignedTxs> for ConsensusEngine<G, M, S, MA, SA>
where
    G: Gossip + Sync + Send,
    M: MemPool<MA>,
    S: Storage<SA>,
    MA: MemPoolAdapter + 'static,
    SA: StorageAdapter + 'static,
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

        let proposal_proof = self
            .adapter
            .get_proof()
            .ok_or_else(|| ProtocolError::from(ConsensusError::MissingProof(epoch_id)))?;

        let cache = self
            .header_cache
            .get(&epoch_id)
            .ok_or_else(|| ProtocolError::from(ConsensusError::MissingEpochHeader(epoch_id)))?;

        let header = EpochHeader {
            chain_id:          self.chain_id.clone(),
            pre_hash:          Hash::from_empty(),
            epoch_id:          tmp,
            timestamp:         time_now(),
            logs_bloom:        cache.logs_bloom,
            order_root:        MerkleRoot::from_empty(),
            confirm_root:      Vec::new(),
            state_root:        cache.state_root.clone(),
            receipt_root:      Vec::new(),
            cycles_used:       0u64,
            proposer:          self.address.clone(),
            proof:             proposal_proof,
            validator_version: 0u64,
            validators:        Vec::new(),
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
        let pill = commit.content;
        let tmp = commit.proof;
        let _proof = Proof {
            epoch_id:   tmp.epoch_id,
            round:      tmp.round,
            epoch_hash: Hash::from_bytes(tmp.epoch_hash)?,
            signature:  tmp.signature.signature,
            bitmap:     tmp.signature.address_bitmap,
        };

        let full_txs = self
            .adapter
            .get_full_txs(ctx.clone(), pill.inner.epoch.ordered_tx_hashes)
            .await?;

        self.adapter
            .save_signed_txs(ctx.clone(), full_txs.clone())
            .await?;

        // TODO: update header cache.

        let status = Status {
            epoch_id:       epoch_id + 1,
            interval:       None,
            authority_list: vec![],
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
                let bytes =
                    serialize(&Proposal::from(sp)).map_err(|e| e as Box<dyn Error + Send>)?;
                (END_GOSSIP_SIGNED_PROPOSAL, bytes)
            }
            OverlordMsg::AggregatedVote(av) => {
                let bytes = serialize(&QC::from(av)).map_err(|e| e as Box<dyn Error + Send>)?;
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
            OverlordMsg::SignedVote(sv) => {
                serialize(&Vote::from(sv)).map_err(|e| e as Box<dyn Error + Send>)?
            }
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

impl<G, M, S, MA, SA> ConsensusEngine<G, M, S, MA, SA>
where
    G: Gossip + Sync + Send,
    M: MemPool<MA>,
    S: Storage<SA>,
    MA: MemPoolAdapter + 'static,
    SA: StorageAdapter + 'static,
{
    fn new(
        chain_id: Hash,
        address: UserAddress,
        cycle_limit: u64,
        adapter: OverlordConsensusAdapter<G, M, S, MA, SA>,
    ) -> Self {
        ConsensusEngine {
            chain_id,
            address,
            cycle_limit,
            exemption_hash: RwLock::new(HashSet::new()),
            state_root: Hash::from_empty(),
            order_root: Hash::from_empty(),
            header_cache: HashMap::new(),
            adapter,
        }
    }
}

struct HeaderCache {
    receipts:       Vec<Receipt>,
    all_cycle_used: u64,
    logs_bloom:     Bloom,
    state_root:     MerkleRoot,
}

impl HeaderCache {
    fn get_all_cycle_used(&self) -> u64 {
        self.all_cycle_used
    }
}

fn time_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
