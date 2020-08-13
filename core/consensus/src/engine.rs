use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::lock::Mutex;
use futures_timer::Delay;
use log::{error, info, warn};
use overlord::error::ConsensusError as OverlordError;
use overlord::types::{Commit, Node, OverlordMsg, Status};
use overlord::{Consensus as Engine, DurationConfig, Wal};
use parking_lot::RwLock;
use rlp::Encodable;

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
use crate::util::{check_list_roots, digest_signed_transactions, time_now, OverlordCrypto};
use crate::wal::SignedTxsWAL;
use crate::ConsensusError;

const RETRY_COMMIT_INTERVAL: u64 = 1000; // 1s
const RETRY_CHECK_ROOT_LIMIT: u8 = 15;
const RETRY_CHECK_ROOT_INTERVAL: u64 = 100; // 100ms

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

    last_commit_time: RwLock<u64>,
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
            error!("[consensus] get_block for {}, error, current_consensus_status.current_height {} != current_consensus_status.current_proof.height, proof :{:?}",
            current_consensus_status.latest_committed_height,
             current_consensus_status.current_proof.height,
            current_consensus_status.current_proof)
        }

        let (ordered_tx_hashes, propose_hashes) = self
            .adapter
            .get_txs_from_mempool(
                ctx.clone(),
                next_height,
                current_consensus_status.cycles_limit,
                current_consensus_status.tx_num_limit,
            )
            .await?
            .clap();
        let signed_txs = self
            .adapter
            .get_full_txs(ctx.clone(), ordered_tx_hashes.clone())
            .await?;
        let order_signed_transactions_hash = digest_signed_transactions(&signed_txs)?;

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
            prev_hash: current_consensus_status.current_hash,
            height: next_height,
            exec_height: current_consensus_status.exec_height,
            timestamp: time_now(),
            order_root: order_root.unwrap_or_else(Hash::from_empty),
            order_signed_transactions_hash,
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
            error!(
                "[consensus] get_block for {}, proof error, proof height {} mismatch",
                header.height,
                header.proof.height,
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
            error!("[consensus-engine]: check_block for overlord receives a proposal, error, block height {}, block {:?}", block.inner.block.header.height,block.inner.block);
        }

        let order_hashes = block.get_ordered_hashes();
        let order_hashes_len = order_hashes.len();
        let exemption = { self.exemption_hash.read().contains(&hash) };
        let sync_tx_hashes = block.get_propose_hashes();

        // If the block is proposed by self, it does not need to check. Get full signed
        // transactions directly.
        if !exemption {
            let current_timestamp = time_now();

            self.adapter
                .verify_block_header(ctx.clone(), block.inner.block.clone())
                .await
                .map_err(|e| {
                    error!(
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

            // verify block timestamp.
            if !validate_timestamp(
                current_timestamp,
                block.inner.block.header.timestamp,
                previous_block.header.timestamp,
            ) {
                return Err(ProtocolError::from(ConsensusError::InvalidTimestamp).into());
            }

            self.adapter
                .verify_proof(
                    ctx.clone(),
                    previous_block.clone(),
                    block.inner.block.header.proof.clone(),
                )
                .await
                .map_err(|e| {
                    error!(
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
                    error!("[consensus] check_block, verify_txs error",);
                    e
                })?;

            // If it is inconsistent with the state of the proposal, we will wait for a
            // period of time.
            let mut check_retry = 0;
            loop {
                match self.check_block_roots(ctx.clone(), &block.inner.block.header) {
                    Ok(()) => break,
                    Err(e) => {
                        if check_retry >= RETRY_CHECK_ROOT_LIMIT {
                            return Err(e.into());
                        }

                        check_retry += 1;
                    }
                }
                Delay::new(Duration::from_millis(RETRY_CHECK_ROOT_INTERVAL)).await;
            }

            let signed_txs = self
                .adapter
                .get_full_txs(ctx.clone(), block.inner.block.ordered_tx_hashes.clone())
                .await?;
            self.check_order_transactions(ctx.clone(), &block.inner.block, &signed_txs)?;

            let adapter = Arc::clone(&self.adapter);
            let ctx_clone = ctx.clone();
            tokio::spawn(async move {
                if let Err(e) = sync_txs(ctx_clone, adapter, sync_tx_hashes).await {
                    error!("Consensus sync block error {}", e);
                }
            });
        }

        info!(
            "[consensus-engine]: check block cost {:?}",
            Instant::now() - time
        );
        let time = Instant::now();
        let txs = self.adapter.get_full_txs(ctx, order_hashes).await?;

        info!(
            "[consensus-engine]: get txs cost {:?}",
            Instant::now() - time
        );
        let time = Instant::now();
        self.txs_wal
            .save(next_height, Hash::from_bytes(hash)?, txs)?;

        info!(
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
            return Err(ProtocolError::from(ConsensusError::LockInSync).into());
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

        if current_height != current_consensus_status.latest_committed_height + 1 {
            return Err(ProtocolError::from(ConsensusError::OutdatedCommit(
                current_height,
                current_consensus_status.latest_committed_height,
            ))
            .into());
        }

        let pill = commit.content.inner;
        let block_hash = Hash::from_bytes(commit.proof.block_hash.clone())?;
        let signature = commit.proof.signature.signature.clone();
        let bitmap = commit.proof.signature.address_bitmap.clone();
        let txs_len = pill.block.ordered_tx_hashes.len();

        // Storage save the latest proof.
        let proof = Proof {
            height: commit.proof.height,
            round: commit.proof.round,
            block_hash: block_hash.clone(),
            signature,
            bitmap,
        };
        common_apm::metrics::consensus::ENGINE_ROUND_GAUGE.set(commit.proof.round as i64);

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

        let block_exec_height = pill.block.header.exec_height;
        let metadata = self.adapter.get_metadata(
            ctx.clone(),
            pill.block.header.state_root.clone(),
            pill.block.header.height,
            pill.block.header.timestamp,
            pill.block.header.proposer.clone(),
        )?;
        info!(
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

        common_apm::metrics::consensus::ENGINE_HEIGHT_GAUGE.set((current_height + 1) as i64);
        common_apm::metrics::consensus::ENGINE_COMMITED_TX_COUNTER.inc_by(txs_len as i64);

        let now = time_now();
        let last_commit_time = *(self.last_commit_time.read());
        let elapsed = (now - last_commit_time) as f64;
        common_apm::metrics::consensus::ENGINE_CONSENSUS_COST_TIME.observe(elapsed / 1e3);
        let mut last_commit_time = self.last_commit_time.write();
        *last_commit_time = now;
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
        logs = "{'pub_key': 'hex::encode(pub_key.clone())'}"
    )]
    async fn transmit_to_relayer(
        &self,
        ctx: Context,
        pub_key: Bytes,
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
                        MessageTarget::Specified(pub_key),
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
                        MessageTarget::Specified(pub_key),
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
            old_block.header.proposer,
        )?;
        let mut old_validators = old_metadata
            .verifier_list
            .into_iter()
            .map(|v| Node {
                address:        v.pub_key.decode(),
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
            last_commit_time: RwLock::new(time_now()),
        }
    }

    #[muta_apm::derive::tracing_span(kind = "consensus.engine")]
    pub async fn exec(
        &self,
        ctx: Context,
        order_root: MerkleRoot,
        height: u64,
        proposer: Address,
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
                proposer,
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
        if status.current_hash != block.prev_hash {
            return Err(ConsensusError::InvalidPrevhash {
                expect: status.current_hash,
                actual: block.prev_hash.clone(),
            }
            .into());
        }

        // check state root
        if status.latest_committed_state_root != block.state_root
            && !status.list_state_root.contains(&block.state_root)
        {
            warn!(
                "invalid status list_state_root, latest {:?}, current list {:?}, block {:?}",
                status.latest_committed_state_root, status.list_state_root, block.state_root
            );
            return Err(ConsensusError::InvalidStatusVec.into());
        }

        // check confirm root
        if !check_list_roots(&status.list_confirm_root, &block.confirm_root) {
            error!(
                "current list confirm root {:?}, block confirm root {:?}",
                status.list_confirm_root, block.confirm_root
            );
            return Err(ConsensusError::InvalidStatusVec.into());
        }

        // check receipt root
        if !check_list_roots(&status.list_receipt_root, &block.receipt_root) {
            error!(
                "current list receipt root {:?}, block receipt root {:?}",
                status.list_receipt_root, block.receipt_root
            );
            return Err(ConsensusError::InvalidStatusVec.into());
        }

        // check cycles used
        if !check_list_roots(&status.list_cycles_used, &block.cycles_used) {
            error!(
                "current list cycles used {:?}, block cycles used {:?}",
                status.list_cycles_used, block.cycles_used
            );
            return Err(ConsensusError::InvalidStatusVec.into());
        }

        Ok(())
    }

    #[muta_apm::derive::tracing_span(
        kind = "consensus.engine",
        logs = "{'txs_len': 'signed_txs.len()'}"
    )]
    fn check_order_transactions(
        &self,
        ctx: Context,
        block: &Block,
        signed_txs: &[SignedTransaction],
    ) -> ProtocolResult<()> {
        let order_root = Merkle::from_hashes(block.ordered_tx_hashes.clone())
            .get_root_hash()
            .unwrap_or_else(Hash::from_empty);
        if order_root != block.header.order_root {
            return Err(ConsensusError::InvalidOrderRoot {
                expect: order_root,
                actual: block.header.order_root.clone(),
            }
            .into());
        }

        let order_signed_transactions_hash = digest_signed_transactions(signed_txs)?;
        if order_signed_transactions_hash != block.header.order_signed_transactions_hash {
            return Err(ConsensusError::InvalidOrderSignedTransactionsHash {
                expect: order_signed_transactions_hash,
                actual: block.header.order_signed_transactions_hash.clone(),
            }
            .into());
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

        let pub_keys = metadata
            .verifier_list
            .iter()
            .map(|v| v.pub_key.decode())
            .collect();
        self.adapter.tag_consensus(Context::new(), pub_keys)?;

        let block_hash = Hash::digest(block.header.encode_fixed()?);

        if block.header.height != proof.height {
            info!("[consensus] update_status for handle_commit, error, before update, block height {}, proof height:{}, proof : {:?}",
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
            error!("[consensus] update_status for handle_commit, error, current_height {} != current_proof.height {}, proof :{:?}",
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

    #[cfg(test)]
    pub fn get_current_status(&self) -> crate::status::CurrentConsensusStatus {
        self.status_agent.to_inner()
    }
}

pub fn generate_new_crypto_map(metadata: Metadata) -> ProtocolResult<HashMap<Bytes, BlsPublicKey>> {
    let mut new_addr_pubkey_map = HashMap::new();
    for validator in metadata.verifier_list.into_iter() {
        let addr = validator.pub_key.decode();
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
            address:        v.pub_key.clone(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect::<Vec<_>>();
    authority.sort();
    authority
}

async fn sync_txs<CA: ConsensusAdapter>(
    ctx: Context,
    adapter: Arc<CA>,
    propose_hashes: Vec<Hash>,
) -> ProtocolResult<()> {
    adapter.sync_txs(ctx, propose_hashes).await
}

fn validate_timestamp(
    current_timestamp: u64,
    proposal_timestamp: u64,
    previous_timestamp: u64,
) -> bool {
    if proposal_timestamp < previous_timestamp {
        return false;
    }

    if proposal_timestamp > current_timestamp {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::validate_timestamp;

    #[test]
    fn test_validate_timestamp() {
        // current 10, proposal 9, previous 8. true
        assert_eq!(validate_timestamp(10, 9, 8), true);

        // current 10, proposal 11, previous 8. true
        assert_eq!(validate_timestamp(10, 11, 8), false);

        // current 10, proposal 9, previous 11. true
        assert_eq!(validate_timestamp(10, 9, 11), false);
    }
}
