use std::collections::HashSet;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{cmp::Eq, sync::Arc};

use async_trait::async_trait;
use bincode::serialize;
use futures::lock::Mutex;
use log::error;
use overlord::types::{Commit, Node, OverlordMsg, Status};
use overlord::Consensus as Engine;
use parking_lot::RwLock;
use rlp::Encodable;

use common_merkle::Merkle;
use protocol::fixed_codec::ProtocolFixedCodec;
use protocol::traits::{ConsensusAdapter, Context, MessageTarget, NodeInfo};
use protocol::types::{
    Address, Epoch, EpochHeader, Hash, MerkleRoot, Pill, Proof, SignedTransaction, UserAddress,
    Validator,
};
use protocol::{Bytes, BytesMut, ProtocolError, ProtocolResult};

use crate::fixed_types::{FixedEpochID, FixedPill, FixedSignedTxs};
use crate::message::{
    END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_RICH_EPOCH_ID, END_GOSSIP_SIGNED_PROPOSAL,
    END_GOSSIP_SIGNED_VOTE, RPC_SYNC_PULL_EPOCH, RPC_SYNC_PULL_TXS,
};
use crate::status::CurrentConsensusStatus;
use crate::{ConsensusError, StatusCacheField};

/// validator is for create new epoch, and authority is for build overlord
/// status.
pub struct ConsensusEngine<Adapter> {
    current_consensus_status: Arc<RwLock<CurrentConsensusStatus>>,
    node_info:                NodeInfo,
    exemption_hash:           RwLock<HashSet<Bytes>>,

    adapter:  Arc<Adapter>,
    pub lock: Mutex<()>,
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
        let tmp_epoch_id = epoch_id;
        let order_root = Merkle::from_hashes(ordered_tx_hashes.clone()).get_root_hash();
        let state_root = current_consensus_status.state_root.last().ok_or_else(|| {
            ProtocolError::from(ConsensusError::StatusErr(StatusCacheField::StateRoot))
        })?;

        let header = EpochHeader {
            chain_id:          self.node_info.chain_id.clone(),
            pre_hash:          current_consensus_status.prev_hash,
            epoch_id:          tmp_epoch_id,
            timestamp:         time_now(),
            logs_bloom:        current_consensus_status.logs_bloom,
            order_root:        order_root.unwrap_or_else(Hash::from_empty),
            confirm_root:      current_consensus_status.confirm_root,
            state_root:        state_root.to_owned(),
            receipt_root:      current_consensus_status.receipt_root.clone(),
            cycles_used:       current_consensus_status.cycles_used,
            proposer:          self.node_info.self_address.clone(),
            proof:             current_consensus_status.proof.clone(),
            validator_version: 0u64,
            validators:        current_consensus_status.validators.clone(),
        };
        let epoch = Epoch {
            header,
            ordered_tx_hashes,
        };

        let pill = Pill {
            epoch,
            propose_hashes,
        };
        let fixed_pill = FixedPill {
            inner: pill.clone(),
        };
        let hash = Hash::digest(pill.encode_fixed()?).as_bytes();
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
            let hash = BytesMut::from(hash.as_ref()).freeze();
            set.contains(&hash)
        };
        // If the epoch is proposed by self, it does not need to check. Get full signed
        // transactions directly.
        if !exemption {
            self.check_epoch_roots(&epoch.inner.epoch.header)?;

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

        let epoch_hash = BytesMut::from(commit.proof.epoch_hash.as_ref()).freeze();
        let signature = BytesMut::from(commit.proof.signature.signature.as_ref()).freeze();
        let bitmap = BytesMut::from(commit.proof.signature.address_bitmap.as_ref()).freeze();

        // Sorage save the lastest proof.
        let proof = Proof {
            epoch_id: commit.proof.epoch_id,
            round: commit.proof.round,
            epoch_hash: Hash::from_bytes(epoch_hash)?,
            signature,
            bitmap,
        };

        self.adapter.save_proof(ctx.clone(), proof.clone()).await?;

        // Get full transactions from mempool temporarily.
        // Storage save the signed transactions.
        let ordered_tx_hashes = pill.epoch.ordered_tx_hashes.clone();
        let full_txs = self
            .adapter
            .get_full_txs(ctx.clone(), ordered_tx_hashes.clone())
            .await?;

        self.adapter
            .flush_mempool(ctx.clone(), ordered_tx_hashes.clone())
            .await?;

        // Execute transactions
        self.exec(
            pill.epoch.header.order_root.clone(),
            epoch_id,
            Address::User(pill.epoch.header.proposer.clone()),
            full_txs.clone(),
        )
        .await?;

        // Broadcast rich epoch ID
        let msg = serialize(&FixedEpochID::new(epoch_id + 1)).map_err(|_| {
            ProtocolError::from(ConsensusError::Other(
                "Encode rich epoch ID error".to_string(),
            ))
        })?;

        self.update_status(epoch_id, pill.epoch, proof, full_txs)
            .await?;

        self.adapter
            .transmit(
                ctx.clone(),
                msg,
                END_GOSSIP_RICH_EPOCH_ID,
                MessageTarget::Broadcast,
            )
            .await?;

        let mut set = self.exemption_hash.write();
        set.clear();
        let current_consensus_status = self.current_consensus_status.read();
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
        match msg {
            OverlordMsg::SignedVote(sv) => {
                let msg = sv.rlp_bytes();
                self.adapter
                    .transmit(
                        ctx,
                        msg,
                        END_GOSSIP_SIGNED_VOTE,
                        MessageTarget::Specified(UserAddress::from_bytes(addr)?),
                    )
                    .await?;
            }
            OverlordMsg::AggregatedVote(av) => {
                let msg = av.rlp_bytes();
                self.adapter
                    .transmit(
                        ctx,
                        msg,
                        END_GOSSIP_AGGREGATED_VOTE,
                        MessageTarget::Specified(UserAddress::from_bytes(addr)?),
                    )
                    .await?;
            }
            _ => unreachable!(),
        };
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

    pub async fn get_current_epoch_id(&self, ctx: Context) -> ProtocolResult<u64> {
        self.adapter.get_current_epoch_id(ctx).await
    }

    pub async fn pull_epoch(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        self.adapter
            .pull_epoch(ctx, epoch_id, RPC_SYNC_PULL_EPOCH)
            .await
    }

    pub async fn pull_txs(
        &self,
        ctx: Context,
        hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        self.adapter.pull_txs(ctx, hashes, RPC_SYNC_PULL_TXS).await
    }

    pub async fn get_epoch_by_id(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        self.adapter.get_epoch_by_id(ctx, epoch_id).await
    }

    pub async fn exec(
        &self,
        order_root: MerkleRoot,
        epoch_id: u64,
        address: Address,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let status = { self.current_consensus_status.read().clone() };

        self.adapter
            .execute(
                self.node_info.clone(),
                order_root,
                epoch_id,
                status.cycles_price,
                address,
                txs,
            )
            .await
    }

    pub fn get_exec_epoch_id(&self) -> u64 {
        let status = self.current_consensus_status.read();
        status.exec_epoch_id
    }

    fn check_epoch_roots(&self, epoch: &EpochHeader) -> ProtocolResult<()> {
        let status = self.current_consensus_status.read().clone();

        // check previous hash
        if status.prev_hash != epoch.pre_hash {
            error!(
                "cache previous hash {:?}, epoch previous hash {:?}",
                status.prev_hash, epoch.pre_hash
            );
            return Err(ConsensusError::CheckEpochErr(StatusCacheField::PrevHash).into());
        }

        // check state root
        if !status.state_root.contains(&epoch.state_root) {
            error!(
                "cache state root {:?}, epoch state root {:?}",
                status.state_root, epoch.state_root
            );
            return Err(ConsensusError::CheckEpochErr(StatusCacheField::StateRoot).into());
        }

        // check confirm root
        if !check_vec_roots(&status.confirm_root, &epoch.confirm_root) {
            error!(
                "cache confirm root {:?}, epoch confirm root {:?}",
                status.confirm_root, epoch.confirm_root
            );
            return Err(ConsensusError::CheckEpochErr(StatusCacheField::ConfirmRoot).into());
        }

        // check receipt root
        if !check_vec_roots(&status.receipt_root, &epoch.receipt_root) {
            error!(
                "cache receipt root {:?}, epoch receipt root {:?}",
                status.receipt_root, epoch.receipt_root
            );
            return Err(ConsensusError::CheckEpochErr(StatusCacheField::ReceiptRoot).into());
        }

        // check cycles used
        if !check_vec_roots(&status.cycles_used, &epoch.cycles_used) {
            error!(
                "cache cycles used {:?}, epoch cycles used {:?}",
                status.cycles_used, epoch.cycles_used
            );
            return Err(ConsensusError::CheckEpochErr(StatusCacheField::CyclesUsed).into());
        }

        Ok(())
    }

    /// **TODO:** parallelism
    /// After get the signed transactions:
    /// 1. Execute the signed transactions.
    /// 2. Save the signed transactions.
    /// 3. Save the latest proof.
    /// 4. Save the new epoch.
    /// 5. Save the receipt.
    pub async fn update_status(
        &self,
        epoch_id: u64,
        epoch: Epoch,
        proof: Proof,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        // Save signed transactions
        self.adapter.save_signed_txs(Context::new(), txs).await?;

        // Save the epoch.
        self.adapter
            .save_epoch(Context::new(), epoch.clone())
            .await?;

        let prev_hash = Hash::digest(epoch.encode_fixed()?);
        {
            let mut current_consensus_status = self.current_consensus_status.write();
            current_consensus_status.update_after_commit(epoch_id + 1, epoch, prev_hash, proof)?;
        }
        //     current_consensus_status.epoch_id = epoch_id + 1;
        //     current_consensus_status.prev_hash = prev_hash;
        //     current_consensus_status.proof = proof;

        //     // Update state root
        //     current_consensus_status.state_root = exec_resp.state_root.clone();

        //     // Update order root
        //     let ordered_root = Merkle::from_hashes(epoch.ordered_tx_hashes.clone())
        //         .get_root_hash()
        //         .unwrap_or_else(Hash::from_empty);
        //     current_consensus_status.order_root = ordered_root.clone();

        //     // Update confirm root
        //     current_consensus_status.confirm_root = vec![ordered_root];

        //     // Update receipt root
        //     current_consensus_status.receipt_root = {
        //         let receipt_root = Merkle::from_hashes(
        //             exec_resp
        //                 .receipts
        //                 .iter()
        //                 .map(|receipt|
        // Hash::digest(receipt.to_owned().encode_fixed().unwrap()))
        // .collect::<Vec<_>>(),         )
        //         .get_root_hash()
        //         .unwrap_or_else(Hash::from_empty);
        //         vec![receipt_root]
        //     };
        // }
        Ok(())
    }

    pub async fn save_proof(&self, ctx: Context, proof: Proof) -> ProtocolResult<()> {
        self.adapter.save_proof(ctx, proof).await
    }

    pub fn get_current_interval(&self) -> u64 {
        let current_consensus_status = self.current_consensus_status.read();
        current_consensus_status.consensus_interval
    }

    pub fn get_current_authority_list(&self) -> Vec<Node> {
        let current_consensus_status = self.current_consensus_status.read();
        covert_to_overlord_authority(&current_consensus_status.validators)
    }

    pub fn get_current_state_root(&self, epoch_id: u64) -> ProtocolResult<Option<MerkleRoot>> {
        let current_consensus_status = self.current_consensus_status.read();
        if epoch_id == current_consensus_status.exec_epoch_id {
            let state_root = current_consensus_status
                .state_root
                .last()
                .ok_or_else(|| ConsensusError::StatusErr(StatusCacheField::StateRoot))?;
            return Ok(Some(state_root.clone()));
        }
        Ok(None)
    }

    pub fn check_state_root(&self, state_root: &MerkleRoot) -> bool {
        let current_consensus_status = self.current_consensus_status.read();
        current_consensus_status.state_root.contains(state_root)
    }

    pub fn get_current_prev_hash(&self) -> Hash {
        let current_consensus_status = self.current_consensus_status.read();
        current_consensus_status.prev_hash.clone()
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

pub fn check_vec_roots<T: Eq>(cache_roots: &[T], epoch_roots: &[T]) -> bool {
    epoch_roots.len() <= cache_roots.len()
        && cache_roots
            .iter()
            .zip(epoch_roots.iter())
            .all(|(c_root, e_root)| c_root == e_root)
}

fn time_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod test {
    use super::check_vec_roots;

    #[test]
    fn test_zip_roots() {
        let roots_1 = vec![1, 2, 3, 4, 5];
        let roots_2 = vec![1, 2, 3];
        let roots_3 = vec![];
        let roots_4 = vec![1, 2];
        let roots_5 = vec![3, 4, 5, 6, 8];

        assert!(check_vec_roots(&roots_1, &roots_2));
        assert!(!check_vec_roots(&roots_3, &roots_2));
        assert!(!check_vec_roots(&roots_4, &roots_2));
        assert!(!check_vec_roots(&roots_5, &roots_2));
    }
}
