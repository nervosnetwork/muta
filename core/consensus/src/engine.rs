use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures::lock::Mutex;
use futures_timer::Delay;
use log::error;
use moodyblues_sdk::trace;
use overlord::error::ConsensusError as OverlordError;
use overlord::types::{Commit, Node, OverlordMsg, Status};
use overlord::{Consensus as Engine, DurationConfig, Wal};
use parking_lot::RwLock;
use rlp::Encodable;
use serde_json::json;

use common_apm::muta_apm;
use common_crypto::BlsPublicKey;
use common_merkle::Merkle;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{ConsensusAdapter, Context, MessageTarget, NodeInfo, TrustFeedback};
use protocol::types::{
    Address, Block, BlockHeader, Hash, MerkleRoot, Metadata, Pill, Proof, SignedTransaction,
    Validator,
};
use protocol::{Bytes, ProtocolError, ProtocolResult};

use crate::fixed_types::FixedPill;
use crate::message::{
    END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_CHOKE, END_GOSSIP_SIGNED_PROPOSAL,
    END_GOSSIP_SIGNED_VOTE,
};
use crate::status::StatusAgent;
use crate::util::{check_list_roots, OverlordCrypto};
use crate::wal::SignedTxsWAL;
use crate::ConsensusError;

const RETRY_COMMIT_INTERVAL: u64 = 1000;

/// validator is for create new block, and authority is for build overlord
/// status.
pub struct ConsensusEngine<Adapter> {
    status_agent:   StatusAgent,
    node_info:      NodeInfo,
    exemption_hash: RwLock<HashSet<Bytes>>,

    adapter: Arc<Adapter>,
    txs_wal: Arc<SignedTxsWAL>,
    crypto:  Arc<OverlordCrypto>,
    lock:    Arc<Mutex<()>>,
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Engine<FixedPill> for ConsensusEngine<Adapter> {
    #[muta_apm::derive::tracing_span(
        kind = "consensus.engine",
        logs = "{'next_height': 'next_height'}"
    )]
    async fn get_block(
        &self,
        ctx: Context,
        next_height: u64,
    ) -> Result<(FixedPill, Bytes), Box<dyn Error + Send>> {
        let current_consensus_status = self.status_agent.to_inner();

        if current_consensus_status.latest_committed_height
            != current_consensus_status.current_proof.height
        {
            log::error!("[consensus] get_block for {}, error, current_consensus_status.current_height {} != current_consensus_status.current_proof.height, proof :{:?}",
            current_consensus_status.latest_committed_height,
             current_consensus_status.current_proof.height,
            current_consensus_status.current_proof)
        }

        let (ordered_tx_hashes, propose_hashes) = self
            .adapter
            .get_txs_from_mempool(
                ctx,
                next_height,
                current_consensus_status.cycles_limit,
                current_consensus_status.tx_num_limit,
            )
            .await?
            .clap();

        if current_consensus_status.latest_committed_height != next_height - 1 {
            return Err(ProtocolError::from(ConsensusError::MissingBlockHeader(
                current_consensus_status.latest_committed_height,
            ))
            .into());
        }

        let order_root = Merkle::from_hashes(ordered_tx_hashes.clone()).get_root_hash();

        let state_root = current_consensus_status.get_latest_state_root();

        let header = BlockHeader {
            chain_id: self.node_info.chain_id.clone(),
            pre_hash: current_consensus_status.current_hash,
            height: next_height,
            exec_height: current_consensus_status.exec_height,
            timestamp: time_now(),
            logs_bloom: current_consensus_status.list_logs_bloom,
            order_root: order_root.unwrap_or_else(Hash::from_empty),
            confirm_root: current_consensus_status.list_confirm_root,
            state_root,
            receipt_root: current_consensus_status.list_receipt_root.clone(),
            cycles_used: current_consensus_status.list_cycles_used,
            proposer: self.node_info.self_address.clone(),
            proof: current_consensus_status.current_proof.clone(),
            validator_version: 0u64,
            validators: current_consensus_status.validators.clone(),
        };

        if header.height != header.proof.height + 1 {
            log::error!(
                "[consensus] get_block for {}, proof error, proof height mismatch, block : {:?}",
                header.height,
                header.clone(),
            );
        }

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
        let hash = Hash::digest(pill.block.header.encode_fixed()?).as_bytes();
        let mut set = self.exemption_hash.write();
        set.insert(hash.clone());

        Ok((fixed_pill, hash))
    }

    #[muta_apm::derive::tracing_span(
        kind = "consensus.engine",
        logs = "{'next_height': 'next_height', 'hash':
    'Hash::from_bytes(hash.clone()).unwrap().as_hex()', 'txs_len':
    'block.inner.block.ordered_tx_hashes.len()'}"
    )]
    async fn check_block(
        &self,
        ctx: Context,
        next_height: u64,
        hash: Bytes,
        block: FixedPill,
    ) -> Result<(), Box<dyn Error + Send>> {
        let time = Instant::now();

        if block.inner.block.header.height != block.inner.block.header.proof.height + 1 {
            log::error!("[consensus-engine]: check_block for overlord receives a proposal, error, block height {}, block {:?}", block.inner.block.header.height,block.inner.block);
        }

        let order_hashes = block.get_ordered_hashes();
        let order_hashes_len = order_hashes.len();
        let exemption = { self.exemption_hash.read().contains(&hash) };
        let sync_tx_hashes = block.get_propose_hashes();

        // If the block is proposed by self, it does not need to check. Get full signed
        // transactions directly.
        if !exemption {
            self.check_block_roots(ctx.clone(), &block.inner.block.header)?;

            self.adapter
                .verify_block_header(ctx.clone(), block.inner.block.clone())
                .await
                .map_err(|e| {
                    log::error!(
                        "[consensus] check_block, verify_block_header error, block header: {:?}",
                        block.inner.block.header
                    );
                    e
                })?;

            // verify the proof in the block for previous block
            // skip to get previous proof to compare because the node may just comes from
            // sync and waste a delay of read
            let previous_block = self
                .adapter
                .get_block_by_height(ctx.clone(), block.inner.block.header.height - 1)
                .await?;

            self.adapter
                .verify_proof(
                    ctx.clone(),
                    previous_block.clone(),
                    block.inner.block.header.proof.clone(),
                )
                .await
                .map_err(|e| {
                    log::error!(
                        "[consensus] check_block, verify_proof error, previous block header: {:?}, proof: {:?}",
                        previous_block.header,
                        block.inner.block.header.proof
                    );
                    e
                })?;

            self.adapter
                .verify_txs(
                    ctx.clone(),
                    block.inner.block.header.height,
                    block.inner.block.ordered_tx_hashes.clone(),
                )
                .await
                .map_err(|e| {
                    log::error!("[consensus] check_block, verify_txs error",);
                    e
                })?;

            let adapter = Arc::clone(&self.adapter);
            let ctx_clone = ctx.clone();
            tokio::spawn(async move {
                if let Err(e) = sync_txs(ctx_clone, adapter, sync_tx_hashes).await {
                    error!("Consensus sync block error {}", e);
                }
            });
        }

        log::info!(
            "[consensus-engine]: check block cost {:?}",
            Instant::now() - time
        );
        let time = Instant::now();
        let txs = self.adapter.get_full_txs(ctx, order_hashes).await?;

        log::info!(
            "[consensus-engine]: get txs cost {:?}",
            Instant::now() - time
        );
        let time = Instant::now();
        self.txs_wal
            .save(next_height, Hash::from_bytes(hash)?, txs)?;

        log::info!(
            "[consensus-engine]: write wal cost {:?} order_hashes_len {:?}",
            time.elapsed(),
            order_hashes_len
        );
        Ok(())
    }

    /// **TODO:** the overlord interface and process needs to be changed.
    /// Get the `FixedSignedTxs` from the argument rather than get it from
    /// mempool.
    #[muta_apm::derive::tracing_span(
        kind = "consensus.engine",
        logs = "{'current_height': 'current_height', 'txs_len':
    'commit.content.inner.block.ordered_tx_hashes.len()'}"
    )]
    async fn commit(
        &self,
        ctx: Context,
        current_height: u64,
        commit: Commit<FixedPill>,
    ) -> Result<Status, Box<dyn Error + Send>> {
        let lock = self.lock.try_lock();

        if lock.is_none() {
            return Err(
                ProtocolError::from(ConsensusError::Other("lock in sync".to_string())).into(),
            );
        }

        let current_consensus_status = self.status_agent.to_inner();
        if current_consensus_status.exec_height == current_height {
            let status = Status {
                height:         current_height + 1,
                interval:       Some(current_consensus_status.consensus_interval),
                timer_config:   Some(DurationConfig {
                    propose_ratio:   current_consensus_status.propose_ratio,
                    prevote_ratio:   current_consensus_status.prevote_ratio,
                    precommit_ratio: current_consensus_status.precommit_ratio,
                    brake_ratio:     current_consensus_status.brake_ratio,
                }),
                authority_list: covert_to_overlord_authority(&current_consensus_status.validators),
            };
            return Ok(status);
        }

        let pill = commit.content.inner;
        let block_hash = Hash::from_bytes(commit.proof.block_hash.clone())?;
        let signature = commit.proof.signature.signature.clone();
        let bitmap = commit.proof.signature.address_bitmap.clone();

        // Storage save the latest proof.
        let proof = Proof {
            height: commit.proof.height,
            round: commit.proof.round,
            block_hash: block_hash.clone(),
            signature,
            bitmap,
        };
        common_apm::metrics::consensus::CONSENSUS_ROUND_HISTOGRAM_VEC_STATIC
            .round
            .observe(proof.round as f64);

        self.adapter.save_proof(ctx.clone(), proof.clone()).await?;

        // Get full transactions from mempool. If is error, try get from wal.
        let ordered_tx_hashes = pill.block.ordered_tx_hashes.clone();
        let signed_txs = match self
            .adapter
            .get_full_txs(ctx.clone(), ordered_tx_hashes.clone())
            .await
        {
            Ok(txs) => txs,
            Err(_) => self.txs_wal.load(current_height, block_hash)?,
        };

        // Execute transactions
        loop {
            if self
                .exec(
                    ctx.clone(),
                    pill.block.header.order_root.clone(),
                    current_height,
                    pill.block.header.proposer.clone(),
                    pill.block.header.timestamp,
                    Hash::digest(pill.block.header.encode_fixed()?),
                    signed_txs.clone(),
                )
                .await
                .is_ok()
            {
                break;
            } else {
                Delay::new(Duration::from_millis(RETRY_COMMIT_INTERVAL)).await;
            }
        }

        trace_block(&pill.block);
        let block_exec_height = pill.block.header.exec_height;
        let metadata = self.adapter.get_metadata(
            ctx.clone(),
            pill.block.header.state_root.clone(),
            pill.block.header.height,
            pill.block.header.timestamp,
        )?;
        log::info!(
            "[consensus]: validator of height {} is {:?}",
            current_height + 1,
            metadata.verifier_list
        );

        self.update_status(metadata, pill.block, proof, signed_txs)
            .await?;

        self.adapter
            .flush_mempool(ctx.clone(), &ordered_tx_hashes)
            .await?;

        self.adapter
            .broadcast_height(ctx.clone(), current_height)
            .await?;
        self.txs_wal.remove(block_exec_height)?;

        let mut set = self.exemption_hash.write();
        set.clear();

        let current_consensus_status = self.status_agent.to_inner();
        let status = Status {
            height:         current_height + 1,
            interval:       Some(current_consensus_status.consensus_interval),
            timer_config:   Some(DurationConfig {
                propose_ratio:   current_consensus_status.propose_ratio,
                prevote_ratio:   current_consensus_status.prevote_ratio,
                precommit_ratio: current_consensus_status.precommit_ratio,
                brake_ratio:     current_consensus_status.brake_ratio,
            }),
            authority_list: covert_to_overlord_authority(&current_consensus_status.validators),
        };

        Ok(status)
    }

    /// Only signed proposal and aggregated vote will be broadcast to others.
    #[muta_apm::derive::tracing_span(kind = "consensus.engine")]
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

            OverlordMsg::SignedChoke(sc) => {
                let bytes = sc.rlp_bytes();
                (END_GOSSIP_SIGNED_CHOKE, bytes)
            }

            _ => unreachable!(),
        };

        self.adapter
            .transmit(ctx, msg, end, MessageTarget::Broadcast)
            .await?;
        Ok(())
    }

    /// Only signed vote will be transmit to the relayer.
    #[muta_apm::derive::tracing_span(
        kind = "consensus.engine",
        logs = "{'address':
    'Address::from_bytes(addr.clone()).unwrap().as_hex()'}"
    )]
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
    #[muta_apm::derive::tracing_span(
        kind = "consensus.engine",
        logs = "{'next_height': 'next_height'}"
    )]
    async fn get_authority_list(
        &self,
        ctx: Context,
        next_height: u64,
    ) -> Result<Vec<Node>, Box<dyn Error + Send>> {
        if next_height == 0 {
            return Ok(vec![]);
        }

        let old_block = self
            .adapter
            .get_block_by_height(ctx.clone(), next_height - 1)
            .await?;
        let old_metadata = self.adapter.get_metadata(
            ctx.clone(),
            old_block.header.state_root.clone(),
            old_block.header.timestamp,
            old_block.header.height,
        )?;
        let mut old_validators = old_metadata
            .verifier_list
            .into_iter()
            .map(|v| Node {
                address:        v.address.as_bytes(),
                propose_weight: v.propose_weight,
                vote_weight:    v.vote_weight,
            })
            .collect::<Vec<_>>();
        old_validators.sort();
        Ok(old_validators)
    }

    fn report_error(&self, ctx: Context, err: OverlordError) {
        match err {
            OverlordError::CryptoErr(_) | OverlordError::AggregatedSignatureErr(_) => self
                .adapter
                .report_bad(ctx, TrustFeedback::Worse(err.to_string())),
            _ => (),
        }
    }
}

#[async_trait]
impl<Adapter: ConsensusAdapter + 'static> Wal for ConsensusEngine<Adapter> {
    async fn save(&self, info: Bytes) -> Result<(), Box<dyn Error + Send>> {
        self.adapter
            .save_overlord_wal(Context::new(), info)
            .await
            .map_err(|e| ProtocolError::from(ConsensusError::Other(e.to_string())))?;
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
        wal: Arc<SignedTxsWAL>,
        adapter: Arc<Adapter>,
        crypto: Arc<OverlordCrypto>,
        lock: Arc<Mutex<()>>,
    ) -> Self {
        Self {
            status_agent,
            node_info,
            exemption_hash: RwLock::new(HashSet::new()),
            txs_wal: wal,
            adapter,
            crypto,
            lock,
        }
    }

    #[muta_apm::derive::tracing_span(kind = "consensus.engine")]
    pub async fn exec(
        &self,
        ctx: Context,
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
                ctx,
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

    #[muta_apm::derive::tracing_span(kind = "consensus.engine")]
    fn check_block_roots(&self, ctx: Context, block: &BlockHeader) -> ProtocolResult<()> {
        let status = self.status_agent.to_inner();

        // check previous hash
        if status.current_hash != block.pre_hash {
            trace::error(
                "check_block_prev_hash_diff".to_string(),
                Some(json!({
                    "next block prev_hash": block.pre_hash.as_hex(),
                    "status current hash": status.current_hash.as_hex(),
                })),
            );
            return Err(ConsensusError::InvalidPrevhash {
                expect: status.current_hash,
                actual: block.pre_hash.clone(),
            }
            .into());
        }

        // check state root
        if status.latest_committed_state_root != block.state_root
            && !status.list_state_root.contains(&block.state_root)
        {
            trace::error(
                "check_block_state_root_diff".to_string(),
                Some(json!({
                    "block_state_root": block.state_root.as_hex(),
                    "current_list_state_root": status.list_state_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                })),
            );
            error!(
                "invalid status list_state_root, latest {:?}, current list {:?}, block {:?}",
                status.latest_committed_state_root, status.list_state_root, block.state_root
            );
            return Err(ConsensusError::InvalidStatusVec.into());
        }

        // check confirm root
        if !check_list_roots(&status.list_confirm_root, &block.confirm_root) {
            trace::error(
                "check_block_confirm_root_diff".to_string(),
                Some(json!({
                    "block_state_root": block.confirm_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                    "current_list_confirm_root": status.list_confirm_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                })),
            );
            error!(
                "current list confirm root {:?}, block confirm root {:?}",
                status.list_confirm_root, block.confirm_root
            );
            return Err(ConsensusError::InvalidStatusVec.into());
        }

        // check receipt root
        if !check_list_roots(&status.list_receipt_root, &block.receipt_root) {
            trace::error(
                "check_block_receipt_root_diff".to_string(),
                Some(json!({
                    "block_state_root": block.receipt_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                    "current_list_receipt_root": status.list_receipt_root.iter().map(|root| root.as_hex()).collect::<Vec<_>>(),
                })),
            );
            error!(
                "current list receipt root {:?}, block receipt root {:?}",
                status.list_receipt_root, block.receipt_root
            );
            return Err(ConsensusError::InvalidStatusVec.into());
        }

        // check cycles used
        if !check_list_roots(&status.list_cycles_used, &block.cycles_used) {
            trace::error(
                "check_block_cycle_used_diff".to_string(),
                Some(json!({
                    "block_state_root": block.cycles_used,
                    "current_list_cycles_root": status.list_cycles_used,
                })),
            );
            error!(
                "current list cycles used {:?}, block cycles used {:?}",
                status.list_cycles_used, block.cycles_used
            );
            return Err(ConsensusError::InvalidStatusVec.into());
        }

        // check logs bloom
        if !check_list_roots(&status.list_logs_bloom, &block.logs_bloom) {
            trace::error(
                "check_block_logs_bloom_diff".to_string(),
                Some(json!({
                    "block_state_root": block.logs_bloom,
                    "current_list_cycles_root": status.list_logs_bloom,
                })),
            );
            error!(
                "cache logs bloom {:?}, block logs bloom {:?}",
                status
                    .list_logs_bloom
                    .iter()
                    .map(|bloom| bloom.to_low_u64_be())
                    .collect::<Vec<_>>(),
                block
                    .logs_bloom
                    .iter()
                    .map(|bloom| bloom.to_low_u64_be())
                    .collect::<Vec<_>>()
            );
            return Err(ConsensusError::InvalidStatusVec.into());
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
        metadata: Metadata,
        block: Block,
        proof: Proof,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        // Save signed transactions
        self.adapter
            .save_signed_txs(Context::new(), block.header.height, txs)
            .await?;

        // Save the block.
        self.adapter
            .save_block(Context::new(), block.clone())
            .await?;

        // update timeout_gap of mempool
        self.adapter.set_args(
            Context::new(),
            metadata.timeout_gap,
            metadata.cycles_limit,
            metadata.max_tx_size,
        );

        let block_hash = Hash::digest(block.header.encode_fixed()?);

        if block.header.height != proof.height {
            log::info!("[consensus] update_status for handle_commit, error, before update, block height {}, proof height:{}, proof : {:?}",
            block.header.height,
            proof.height,
            proof.clone());
        }

        self.status_agent
            .update_by_committed(metadata.clone(), block, block_hash, proof);

        let committed_status_agent = self.status_agent.to_inner();

        if committed_status_agent.latest_committed_height
            != committed_status_agent.current_proof.height
        {
            log::error!("[consensus] update_status for handle_commit, error, current_height {} != current_proof.height {}, proof :{:?}",
            committed_status_agent.latest_committed_height,
            committed_status_agent.current_proof.height,
            committed_status_agent.current_proof)
        }

        self.update_overlord_crypto(metadata)?;
        Ok(())
    }

    fn update_overlord_crypto(&self, metadata: Metadata) -> ProtocolResult<()> {
        self.crypto.update(generate_new_crypto_map(metadata)?);
        Ok(())
    }
}

pub fn generate_new_crypto_map(metadata: Metadata) -> ProtocolResult<HashMap<Bytes, BlsPublicKey>> {
    let mut new_addr_pubkey_map = HashMap::new();
    for validator in metadata.verifier_list.into_iter() {
        let addr = validator.address.as_bytes();
        let hex_pubkey = hex::decode(validator.bls_pub_key.as_string_trim0x()).map_err(|err| {
            ConsensusError::Other(format!("hex decode metadata bls pubkey error {:?}", err))
        })?;
        let pubkey = BlsPublicKey::try_from(hex_pubkey.as_ref())
            .map_err(|err| ConsensusError::Other(format!("try from bls pubkey error {:?}", err)))?;
        new_addr_pubkey_map.insert(addr, pubkey);
    }
    Ok(new_addr_pubkey_map)
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
        .as_millis() as u64
}

async fn sync_txs<CA: ConsensusAdapter>(
    ctx: Context,
    adapter: Arc<CA>,
    propose_hashes: Vec<Hash>,
) -> ProtocolResult<()> {
    adapter.sync_txs(ctx, propose_hashes).await
}
