use std::collections::HashSet;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{cmp::Eq, sync::Arc};

use async_trait::async_trait;
use futures::lock::Mutex;
use log::error;
use moodyblues_sdk::trace;
use overlord::types::{Commit, Node, OverlordMsg, Status};
use overlord::{Consensus as Engine, Wal};
use parking_lot::RwLock;
use rlp::Encodable;
use serde_json::json;

use common_merkle::Merkle;
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{ConsensusAdapter, Context, MessageCodec, MessageTarget, NodeInfo};
use protocol::types::{
    Address, Block, BlockHeader, Hash, MerkleRoot, Metadata, Pill, Proof, SignedTransaction,
    Validator,
};
use protocol::{Bytes, ProtocolError, ProtocolResult};

use crate::fixed_types::FixedPill;
use crate::message::{
    END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE,
};
use crate::status::StatusAgent;
use crate::{ConsensusError, StatusCacheField};

/// validator is for create new block, and authority is for build overlord
/// status.
pub struct ConsensusEngine<Adapter> {
    status_agent:   StatusAgent,
    node_info:      NodeInfo,
    exemption_hash: RwLock<HashSet<Bytes>>,

    adapter: Arc<Adapter>,
    lock:    Arc<Mutex<()>>,
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Engine<FixedPill> for ConsensusEngine<Adapter> {
    async fn get_block(
        &self,
        ctx: Context,
        height: u64,
    ) -> Result<(FixedPill, Bytes), Box<dyn Error + Send>> {
        let current_consensus_status = self.status_agent.to_inner();

        let (ordered_tx_hashes, propose_hashes) = self
            .adapter
            .get_txs_from_mempool(ctx, height, current_consensus_status.cycles_limit)
            .await?
            .clap();

        if current_consensus_status.height != height {
            return Err(ProtocolError::from(ConsensusError::MissingBlockHeader(height)).into());
        }
        let tmp_height = height;
        let order_root = Merkle::from_hashes(ordered_tx_hashes.clone()).get_root_hash();

        let header = BlockHeader {
            chain_id:          self.node_info.chain_id.clone(),
            pre_hash:          current_consensus_status.prev_hash,
            height:            tmp_height,
            exec_height:       current_consensus_status.exec_height,
            timestamp:         time_now(),
            logs_bloom:        current_consensus_status.logs_bloom,
            order_root:        order_root.unwrap_or_else(Hash::from_empty),
            confirm_root:      current_consensus_status.confirm_root,
            state_root:        current_consensus_status.latest_state_root.clone(),
            receipt_root:      current_consensus_status.receipt_root.clone(),
            cycles_used:       current_consensus_status.cycles_used,
            proposer:          self.node_info.self_address.clone(),
            proof:             current_consensus_status.proof.clone(),
            validator_version: 0u64,
            validators:        current_consensus_status.validators.clone(),
        };
        let block = Block {
            header,
            ordered_tx_hashes,
        };

        let pill = Pill {
            block,
            propose_hashes,
        };
        let fixed_pill = FixedPill {
            inner: pill.clone(),
        };
        let hash = Hash::digest(pill.block.encode_fixed()?).as_bytes();
        let mut set = self.exemption_hash.write();
        set.insert(hash.clone());

        Ok((fixed_pill, hash))
    }

    async fn check_block(
        &self,
        ctx: Context,
        _height: u64,
        hash: Bytes,
        block: FixedPill,
    ) -> Result<(), Box<dyn Error + Send>> {
        let order_hashes = block.get_ordered_hashes();
        let exemption = { self.exemption_hash.read().contains(&hash) };
        let sync_tx_hashes = block.get_propose_hashes();

        // If the block is proposed by self, it does not need to check. Get full signed
        // transactions directly.
        if !exemption {
            self.check_block_roots(&block.inner.block.header)?;
            self.adapter
                .check_txs(ctx.clone(), order_hashes.clone())
                .await?;
        }

        let inner = self.adapter.get_full_txs(ctx.clone(), order_hashes).await?;
        let adapter = Arc::clone(&self.adapter);

        tokio::spawn(async move {
            if let Err(e) = sync_txs(ctx, adapter, sync_tx_hashes).await {
                error!("Consensus sync block error {}", e);
            }
        });
        self.adapter
            .save_wal_transactions(Context::new(), Hash::digest(hash.clone()), inner)
            .await?;
        Ok(())
    }

    /// **TODO:** the overlord interface and process needs to be changed.
    /// Get the `FixedSignedTxs` from the argument rather than get it from
    /// mempool.
    async fn commit(
        &self,
        ctx: Context,
        height: u64,
        commit: Commit<FixedPill>,
    ) -> Result<Status, Box<dyn Error + Send>> {
        let lock = self.lock.try_lock();

        if lock.is_none() {
            return Err(
                ProtocolError::from(ConsensusError::Other("lock in sync".to_string())).into(),
            );
        }

        let current_consensus_status = self.status_agent.to_inner();
        if current_consensus_status.exec_height == height {
            let status = Status {
                height:         height + 1,
                interval:       Some(current_consensus_status.consensus_interval),
                authority_list: covert_to_overlord_authority(&current_consensus_status.validators),
            };
            return Ok(status);
        }

        let pill = commit.content.inner;

        let block_hash = commit.proof.block_hash.clone();
        let signature = commit.proof.signature.signature.clone();
        let bitmap = commit.proof.signature.address_bitmap.clone();

        // Sorage save the lastest proof.
        let proof = Proof {
            height: commit.proof.height,
            round: commit.proof.round,
            block_hash: Hash::from_bytes(block_hash.clone())?,
            signature,
            bitmap,
        };

        self.adapter.save_proof(ctx.clone(), proof.clone()).await?;

        // Get full transactions from mempool. If is error, try get from wal.
        let ordered_tx_hashes = pill.block.ordered_tx_hashes.clone();
        let full_txs = match self
            .adapter
            .get_full_txs(ctx.clone(), ordered_tx_hashes.clone())
            .await
        {
            Ok(txs) => txs,
            Err(_) => {
                self.adapter
                    .load_wal_transactions(ctx.clone(), Hash::digest(block_hash))
                    .await?
            }
        };

        self.adapter
            .flush_mempool(ctx.clone(), &ordered_tx_hashes)
            .await?;

        // Execute transactions
        self.exec(
            pill.block.header.order_root.clone(),
            height,
            pill.block.header.proposer.clone(),
            pill.block.header.timestamp,
            Hash::digest(pill.block.encode_fixed()?),
            full_txs.clone(),
        )
        .await?;

        trace_block(&pill.block);
        let metadata = self.adapter.get_metadata(
            ctx.clone(),
            pill.block.header.state_root.clone(),
            pill.block.header.height,
            pill.block.header.timestamp,
        )?;
        self.update_status(height, metadata, pill.block, proof, full_txs)
            .await?;

        self.adapter.broadcast_height(ctx.clone(), height).await?;

        let mut set = self.exemption_hash.write();
        set.clear();
        let current_consensus_status = self.status_agent.to_inner();
        let status = Status {
            height:         height + 1,
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
                        MessageTarget::Specified(Address::from_bytes(addr)?),
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
                        MessageTarget::Specified(Address::from_bytes(addr)?),
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
        height: u64,
    ) -> Result<Vec<Node>, Box<dyn Error + Send>> {
        let validators = self.adapter.get_last_validators(ctx, height).await?;
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

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Wal for ConsensusEngine<Adapter> {
    async fn save(&self, info: Bytes) -> Result<(), Box<dyn Error + Send>> {
        self.adapter
            .save_overlord_wal(Context::new(), info)
            .await
            .map_err(|e| ProtocolError::from(ConsensusError::WalErr(e.to_string())))?;
        Ok(())
    }

    async fn load(&self) -> Result<Option<Bytes>, Box<dyn Error + Send>> {
        let res = self.adapter.load_overlord_wal(Context::new()).await.ok();
        Ok(res)
    }
}

impl<Adapter: ConsensusAdapter + 'static> ConsensusEngine<Adapter> {
    pub fn new(
        status_agent: StatusAgent,
        node_info: NodeInfo,
        adapter: Arc<Adapter>,
        lock: Arc<Mutex<()>>,
    ) -> Self {
        Self {
            status_agent,
            node_info,
            exemption_hash: RwLock::new(HashSet::new()),
            adapter,
            lock,
        }
    }

    pub async fn exec(
        &self,
        order_root: MerkleRoot,
        height: u64,
        address: Address,
        timestamp: u64,
        block_hash: Hash,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let status = self.status_agent.to_inner();

        self.adapter
            .execute(
                self.node_info.chain_id.clone(),
                order_root,
                height,
                status.cycles_price,
                address,
                block_hash,
                txs,
                status.cycles_limit,
                timestamp,
            )
            .await
    }

    fn check_block_roots(&self, block: &BlockHeader) -> ProtocolResult<()> {
        let status = self.status_agent.to_inner();

        // check previous hash
        if status.prev_hash != block.pre_hash {
            trace::error(
                "check_block_prev_hash_diff".to_string(),
                Some(json!({
                    "block_prev_hash": block.pre_hash.as_hex(),
                    "cache_prev_hash": status.prev_hash.as_hex(),
                })),
            );
            error!(
                "cache previous hash {:?}, block previous hash {:?}",
                status.prev_hash, block.pre_hash
            );
            return Err(ConsensusError::CheckBlockErr(StatusCacheField::PrevHash).into());
        }

        // check state root
        if !status.state_root.contains(&block.state_root) {
            trace::error(
                "check_block_state_root_diff".to_string(),
                Some(json!({
                    "block_state_root": block.state_root.as_hex(),
                    "cache_state_roots": status.state_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                })),
            );
            error!(
                "cache state root {:?}, block state root {:?}",
                status.state_root, block.state_root
            );
            return Err(ConsensusError::CheckBlockErr(StatusCacheField::StateRoot).into());
        }

        // check confirm root
        if !check_vec_roots(&status.confirm_root, &block.confirm_root) {
            trace::error(
                "check_block_confirm_root_diff".to_string(),
                Some(json!({
                    "block_state_root": block.confirm_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                    "cache_state_roots": status.confirm_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                })),
            );
            error!(
                "cache confirm root {:?}, block confirm root {:?}",
                status.confirm_root, block.confirm_root
            );
            return Err(ConsensusError::CheckBlockErr(StatusCacheField::ConfirmRoot).into());
        }

        // check receipt root
        if !check_vec_roots(&status.receipt_root, &block.receipt_root) {
            trace::error(
                "check_block_receipt_root_diff".to_string(),
                Some(json!({
                    "block_state_root": block.receipt_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                    "cache_state_roots": status.receipt_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                })),
            );
            error!(
                "cache receipt root {:?}, block receipt root {:?}",
                status.receipt_root, block.receipt_root
            );
            return Err(ConsensusError::CheckBlockErr(StatusCacheField::ReceiptRoot).into());
        }

        // check cycles used
        if !check_vec_roots(&status.cycles_used, &block.cycles_used) {
            trace::error(
                "check_block_cycle_used_diff".to_string(),
                Some(json!({
                    "block_state_root": block.cycles_used,
                    "cache_state_roots": status.cycles_used,
                })),
            );
            error!(
                "cache cycles used {:?}, block cycles used {:?}",
                status.cycles_used, block.cycles_used
            );
            return Err(ConsensusError::CheckBlockErr(StatusCacheField::CyclesUsed).into());
        }

        Ok(())
    }

    /// After get the signed transactions:
    /// 1. Execute the signed transactions.
    /// 2. Save the signed transactions.
    /// 3. Save the latest proof.
    /// 4. Save the new block.
    /// 5. Save the receipt.
    pub async fn update_status(
        &self,
        height: u64,
        metadata: Metadata,
        block: Block,
        proof: Proof,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        // Save signed transactions
        self.adapter.save_signed_txs(Context::new(), txs).await?;

        // Save the block.
        self.adapter
            .save_block(Context::new(), block.clone())
            .await?;

        let prev_hash = Hash::digest(block.encode_fixed()?);
        self.status_agent
            .update_after_commit(height + 1, metadata, block, prev_hash, proof)?;
        self.save_wal().await
    }

    async fn save_wal(&self) -> ProtocolResult<()> {
        let mut info = self.status_agent.to_inner();
        let wal_info = MessageCodec::encode(&mut info).await?;
        self.adapter.save_muta_wal(Context::new(), wal_info).await
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

pub fn check_vec_roots<T: Eq>(cache_roots: &[T], block_roots: &[T]) -> bool {
    block_roots.len() <= cache_roots.len()
        && cache_roots
            .iter()
            .zip(block_roots.iter())
            .all(|(c_root, e_root)| c_root == e_root)
}

pub fn trace_block(block: &Block) {
    let confirm_roots = block
        .header
        .confirm_root
        .iter()
        .map(|root| root.as_hex())
        .collect::<Vec<_>>();
    let receipt_roots = block
        .header
        .receipt_root
        .iter()
        .map(|root| root.as_hex())
        .collect::<Vec<_>>();

    trace::custom(
        "commit_block".to_string(),
        Some(json!({
            "height": block.header.height,
            "pre_hash": block.header.pre_hash.as_hex(),
            "order_root": block.header.order_root.as_hex(),
            "state_root": block.header.state_root.as_hex(),
            "proposer": block.header.proposer.as_hex(),
            "confirm_roots": confirm_roots,
            "receipt_roots": receipt_roots,
        })),
    );
}

fn time_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn sync_txs<CA: ConsensusAdapter>(
    ctx: Context,
    adapter: Arc<CA>,
    propose_hashes: Vec<Hash>,
) -> ProtocolResult<()> {
    adapter.sync_txs(ctx, propose_hashes).await
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
