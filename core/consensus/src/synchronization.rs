use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::lock::Mutex;
use futures_timer::Delay;

use common_apm::muta_apm;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    Context, ExecutorParams, ExecutorResp, Synchronization, SynchronizationAdapter,
};
use protocol::types::{Block, Hash, Proof, Receipt, SignedTransaction};
use protocol::ProtocolResult;

use crate::engine::generate_new_crypto_map;
use crate::status::{ExecutedInfo, StatusAgent};
use crate::util::{digest_signed_transactions, OverlordCrypto};
use crate::ConsensusError;

const POLLING_BROADCAST: u64 = 2000;
const WAIT_EXECUTION: u64 = 1000;
const ONCE_SYNC_BLOCK_LIMIT: u64 = 50;

#[derive(Clone, Debug)]
pub struct RichBlock {
    pub block: Block,
    pub txs:   Vec<SignedTransaction>,
}

pub struct OverlordSynchronization<Adapter: SynchronizationAdapter> {
    adapter: Arc<Adapter>,
    status:  StatusAgent,
    crypto:  Arc<OverlordCrypto>,
    lock:    Arc<Mutex<()>>,
    syncing: Mutex<()>,

    sync_txs_chunk_size: usize,
}

#[async_trait]
impl<Adapter: SynchronizationAdapter> Synchronization for OverlordSynchronization<Adapter> {
    #[muta_apm::derive::tracing_span(
        kind = "consensus.sync",
        logs = "{'remote_height': 'remote_height'}"
    )]
    async fn receive_remote_block(&self, ctx: Context, remote_height: u64) -> ProtocolResult<()> {
        let syncing_lock = self.syncing.try_lock();
        if syncing_lock.is_none() {
            return Ok(());
        }
        if !self.need_sync(ctx.clone(), remote_height).await? {
            return Ok(());
        }

        // Lock the consensus engine, block commit process.
        let commit_lock = self.lock.try_lock();
        if commit_lock.is_none() {
            return Ok(());
        }

        let current_height = self.status.to_inner().latest_committed_height;

        if remote_height <= current_height {
            return Ok(());
        }

        log::info!(
            "[synchronization]: sync start, remote block height {:?} current block height {:?}",
            remote_height,
            current_height,
        );

        let sync_status_agent = self.init_status_agent().await?;
        let sync_resp = self
            .start_sync(
                ctx.clone(),
                sync_status_agent.clone(),
                current_height,
                remote_height,
            )
            .await;
        let sync_status = sync_status_agent.to_inner();

        if let Err(e) = sync_resp {
            log::error!(
                "[synchronization]: err, current_height {:?} err_msg: {:?}",
                sync_status.latest_committed_height,
                e
            );
            return Err(e);
        }

        log::info!(
            "[synchronization]: sync end, remote block height {:?} current block height {:?} current exec height {:?} current proof height {:?}",
            remote_height,
            sync_status.latest_committed_height,
            sync_status.exec_height,
            sync_status.current_proof.height,
        );

        Ok(())
    }
}

impl<Adapter: SynchronizationAdapter> OverlordSynchronization<Adapter> {
    pub fn new(
        sync_txs_chunk_size: usize,
        adapter: Arc<Adapter>,
        status: StatusAgent,
        crypto: Arc<OverlordCrypto>,
        lock: Arc<Mutex<()>>,
    ) -> Self {
        let syncing = Mutex::new(());

        Self {
            adapter,
            status,
            crypto,
            lock,
            syncing,

            sync_txs_chunk_size,
        }
    }

    pub async fn polling_broadcast(&self) -> ProtocolResult<()> {
        loop {
            let current_height = self.status.to_inner().latest_committed_height;
            if current_height != 0 {
                self.adapter
                    .broadcast_height(Context::new(), current_height)
                    .await?;
            }
            Delay::new(Duration::from_millis(POLLING_BROADCAST)).await;
        }
    }

    #[muta_apm::derive::tracing_span(
        kind = "consensus.sync",
        logs = "{'current_height': 'current_height', 'remote_height': 'remote_height'}"
    )]
    async fn start_sync(
        &self,
        ctx: Context,
        sync_status_agent: StatusAgent,
        current_height: u64,
        remote_height: u64,
    ) -> ProtocolResult<()> {
        let remote_height = if current_height + ONCE_SYNC_BLOCK_LIMIT > remote_height {
            remote_height
        } else {
            current_height + ONCE_SYNC_BLOCK_LIMIT
        };

        let mut current_consented_height = current_height;

        while current_consented_height < remote_height {
            let inst = Instant::now();

            let consenting_height = current_consented_height + 1;
            log::info!(
                "[synchronization]: try syncing block, current_consented_height:{},syncing_height:{}",
                current_consented_height,
                consenting_height
            );

            let consenting_rich_block: RichBlock = self
                .get_rich_block_from_remote(ctx.clone(), consenting_height)
                .await
                .map_err(|e| {
                    log::error!(
                        "[synchronization]: get_rich_block_from_remote error, height: {:?}",
                        consenting_height
                    );
                    e
                })?;

            let consenting_proof: Proof = self
                .adapter
                .get_proof_from_remote(ctx.clone(), consenting_height)
                .await
                .map_err(|e| {
                    log::error!(
                        "[synchronization]: get_proof_from_remote error, height: {:?}",
                        consenting_height
                    );
                    e
                })?;

            self.adapter
                .verify_block_header(ctx.clone(), &consenting_rich_block.block)
                .await
                .map_err(|e| {
                    log::error!(
                        "[synchronization]: verify_block_header error, block header: {:?}",
                        consenting_rich_block.block.header
                    );
                    e
                })?;

            // verify syncing proof
            self.adapter
                .verify_proof(
                    ctx.clone(),
                    &consenting_rich_block.block.header,
                    &consenting_proof,
                )
                .await
                .map_err(|e| {
                    log::error!(
                        "[synchronization]: verify_proof error, syncing block header: {:?}, proof: {:?}",
                        consenting_rich_block.block.header,
                        consenting_proof,
                    );
                    e
                })?;

            // verify previous proof
            let previous_block_header = self
                .adapter
                .get_block_header_by_height(
                    ctx.clone(),
                    consenting_rich_block.block.header.height - 1,
                )
                .await
                .map_err(|e| {
                    log::error!(
                        "[synchronization] get previous block {} error",
                        consenting_rich_block.block.header.height - 1
                    );
                    e
                })?;

            self.adapter
                .verify_proof(
                    ctx.clone(),
                    &previous_block_header,
                    &consenting_rich_block.block.header.proof,
                )
                .await
                .map_err(|e| {
                    log::error!(
                        "[synchronization]: verify_proof error, previous block header: {:?}, proof: {:?}",
                        previous_block_header,
                        consenting_rich_block.block.header.proof
                    );
                    e
                })?;

            let order_signed_transactions_hash =
                digest_signed_transactions(&consenting_rich_block.txs)?;
            if order_signed_transactions_hash
                != consenting_rich_block
                    .block
                    .header
                    .order_signed_transactions_hash
            {
                return Err(ConsensusError::InvalidOrderSignedTransactionsHash {
                    expect: order_signed_transactions_hash,
                    actual: consenting_rich_block
                        .block
                        .header
                        .order_signed_transactions_hash
                        .clone(),
                }
                .into());
            }

            let inst = Instant::now();
            self.commit_block(
                ctx.clone(),
                consenting_rich_block.clone(),
                consenting_proof,
                sync_status_agent.clone(),
            )
            .await
            .map_err(|e| {
                log::error!(
                    "[synchronization]: commit block {} error",
                    consenting_rich_block.block.header.height
                );
                e
            })?;

            self.update_status(ctx.clone(), sync_status_agent.clone())?;
            current_consented_height += 1;

            common_apm::metrics::consensus::ENGINE_SYNC_BLOCK_COUNTER.inc_by(1 as i64);
            common_apm::metrics::consensus::ENGINE_SYNC_BLOCK_HISTOGRAM
                .observe(common_apm::metrics::duration_to_sec(inst.elapsed()));
        }
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "consensus.sync")]
    async fn commit_block(
        &self,
        ctx: Context,
        rich_block: RichBlock,
        proof: Proof,
        status_agent: StatusAgent,
    ) -> ProtocolResult<()> {
        let executor_resp = self
            .exec_block(ctx.clone(), rich_block.clone(), status_agent.clone())
            .await?;
        let block = &rich_block.block;
        let block_hash = Hash::digest(block.header.encode_fixed()?);

        let metadata = self.adapter.get_metadata(
            ctx.clone(),
            block.header.state_root.clone(),
            block.header.height,
            block.header.timestamp,
            block.header.proposer.clone(),
        )?;

        self.crypto
            .update(generate_new_crypto_map(metadata.clone())?);

        self.adapter.set_args(
            ctx.clone(),
            metadata.timeout_gap,
            metadata.cycles_limit,
            metadata.max_tx_size,
        );

        let pub_keys = metadata
            .verifier_list
            .iter()
            .map(|v| v.pub_key.decode())
            .collect();
        self.adapter.tag_consensus(ctx.clone(), pub_keys)?;

        log::info!(
            "[synchronization]: commit_block, committing block header: {}, committing proof:{:?}",
            block.header.clone(),
            proof.clone()
        );

        status_agent.update_by_committed(metadata, block.clone(), block_hash, proof);

        self.save_chain_data(
            ctx.clone(),
            rich_block.txs.clone(),
            executor_resp.receipts.clone(),
            rich_block.block.clone(),
        )
        .await?;

        // If there are transactions in the trasnaction pool that have been on chain
        // after this execution, make sure they are cleaned up.
        self.adapter
            .flush_mempool(ctx.clone(), &rich_block.block.ordered_tx_hashes)
            .await?;

        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "consensus.sync", logs = "{'height': 'height'}")]
    async fn get_rich_block_from_remote(
        &self,
        ctx: Context,
        height: u64,
    ) -> ProtocolResult<RichBlock> {
        let block = self.get_block_from_remote(ctx.clone(), height).await?;

        let mut txs = Vec::with_capacity(block.ordered_tx_hashes.len());

        for tx_hashes in block.ordered_tx_hashes.chunks(self.sync_txs_chunk_size) {
            let remote_txs = self
                .adapter
                .get_txs_from_remote(ctx.clone(), height, &tx_hashes)
                .await?;

            txs.extend(remote_txs);
        }

        Ok(RichBlock { block, txs })
    }

    #[muta_apm::derive::tracing_span(kind = "consensus.sync", logs = "{'height': 'height'}")]
    async fn get_block_from_remote(&self, ctx: Context, height: u64) -> ProtocolResult<Block> {
        self.adapter
            .get_block_from_remote(ctx.clone(), height)
            .await
    }

    #[muta_apm::derive::tracing_span(kind = "consensus.sync", logs = "{'txs_len': 'txs.len()'}")]
    async fn save_chain_data(
        &self,
        ctx: Context,
        txs: Vec<SignedTransaction>,
        receipts: Vec<Receipt>,
        block: Block,
    ) -> ProtocolResult<()> {
        self.adapter
            .save_signed_txs(ctx.clone(), block.header.height, txs)
            .await?;
        self.adapter
            .save_receipts(ctx.clone(), block.header.height, receipts)
            .await?;
        self.adapter
            .save_proof(ctx.clone(), block.header.proof.clone())
            .await?;
        self.adapter.save_block(ctx.clone(), block).await?;
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "consensus.sync")]
    pub async fn exec_block(
        &self,
        ctx: Context,
        rich_block: RichBlock,
        status_agent: StatusAgent,
    ) -> ProtocolResult<ExecutorResp> {
        let current_status = status_agent.to_inner();
        let cycles_limit = current_status.cycles_limit;

        let exec_params = ExecutorParams {
            state_root: current_status.get_latest_state_root(),
            height: rich_block.block.header.height,
            timestamp: rich_block.block.header.timestamp,
            cycles_limit,
            proposer: rich_block.block.header.proposer,
        };
        let resp = self
            .adapter
            .sync_exec(ctx.clone(), &exec_params, &rich_block.txs)?;

        status_agent.update_by_executed(ExecutedInfo::new(
            ctx,
            rich_block.block.header.height,
            rich_block.block.header.order_root,
            resp.clone(),
        ));

        Ok(resp)
    }

    async fn init_status_agent(&self) -> ProtocolResult<StatusAgent> {
        loop {
            let current_status = self.status.to_inner();

            if current_status.exec_height != current_status.latest_committed_height {
                Delay::new(Duration::from_millis(WAIT_EXECUTION)).await;
            } else {
                break;
            }
        }
        let current_status = self.status.to_inner();
        Ok(StatusAgent::new(current_status))
    }

    #[muta_apm::derive::tracing_span(
        kind = "consensus.sync",
        logs = "{'remote_height': 'remote_height'}"
    )]
    async fn need_sync(&self, ctx: Context, remote_height: u64) -> ProtocolResult<bool> {
        let mut current_height = self.status.to_inner().latest_committed_height;
        if remote_height == 0 {
            return Ok(false);
        }

        if remote_height <= current_height {
            return Ok(false);
        }

        if current_height == remote_height - 1 {
            let status = self.status.to_inner();
            Delay::new(Duration::from_millis(status.consensus_interval)).await;

            current_height = self.status.to_inner().latest_committed_height;
            if current_height == remote_height {
                return Ok(false);
            }
        }

        let block = self
            .get_block_from_remote(ctx.clone(), remote_height)
            .await?;

        log::debug!(
            "[synchronization] get block from remote success {:?} ",
            remote_height
        );

        if block.header.height != remote_height {
            log::error!("[synchronization]: block that doesn't match is found");
            return Ok(false);
        }

        Ok(true)
    }

    fn update_status(&self, ctx: Context, sync_status_agent: StatusAgent) -> ProtocolResult<()> {
        let sync_status = sync_status_agent.to_inner();

        self.status.replace(sync_status.clone());
        self.adapter.update_status(
            ctx,
            sync_status.latest_committed_height,
            sync_status.consensus_interval,
            sync_status.propose_ratio,
            sync_status.prevote_ratio,
            sync_status.precommit_ratio,
            sync_status.brake_ratio,
            sync_status.validators,
        )?;

        log::info!(
            "[synchronization]: synced block, status: height:{}, exec_height:{}, proof_height:{}",
            sync_status.latest_committed_height,
            sync_status.exec_height,
            sync_status.current_proof.height
        );
        Ok(())
    }
}
