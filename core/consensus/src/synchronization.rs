use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::lock::Mutex;
use futures_timer::Delay;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    Context, ExecutorParams, ExecutorResp, Synchronization, SynchronizationAdapter,
};
use protocol::types::{Block, Hash, Receipt, SignedTransaction};
use protocol::ProtocolResult;

use crate::status::{ExecutedInfo, StatusAgent};
use crate::ConsensusError;

const POLLING_BROADCAST: u64 = 2000;
const WAIT_EXECUTION: u64 = 1000;

#[derive(Clone, Debug)]
pub struct RichBlock {
    pub block: Block,
    pub txs:   Vec<SignedTransaction>,
}

pub struct OverlordSynchronization<Adapter: SynchronizationAdapter> {
    adapter: Arc<Adapter>,
    status:  StatusAgent,
    lock:    Arc<Mutex<()>>,
    syncing: Mutex<()>,

    sync_txs_chunk_size: usize,
}

#[async_trait]
impl<Adapter: SynchronizationAdapter> Synchronization for OverlordSynchronization<Adapter> {
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

        let current_height = self.status.to_inner().current_height;

        if remote_height <= current_height {
            return Ok(());
        }

        log::info!(
            "[synchronization]: start, remote block height {:?} current block height {:?}",
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
                sync_status.current_height,
                e
            );
        }

        self.status.replace(sync_status.clone());
        self.adapter.update_status(
            ctx.clone(),
            sync_status.current_height,
            sync_status.consensus_interval,
            sync_status.propose_ratio,
            sync_status.prevote_ratio,
            sync_status.precommit_ratio,
            sync_status.brake_ratio,
            sync_status.validators,
        )?;

        log::info!(
            "[synchronization]: end, remote block height {:?} current block height {:?}",
            remote_height,
            current_height,
        );
        Ok(())
    }
}

impl<Adapter: SynchronizationAdapter> OverlordSynchronization<Adapter> {
    pub fn new(
        sync_txs_chunk_size: usize,
        adapter: Arc<Adapter>,
        status: StatusAgent,
        lock: Arc<Mutex<()>>,
    ) -> Self {
        let syncing = Mutex::new(());

        Self {
            adapter,
            status,
            lock,
            syncing,

            sync_txs_chunk_size,
        }
    }

    pub async fn polling_broadcast(&self) -> ProtocolResult<()> {
        loop {
            let current_height = self.status.to_inner().current_height;
            if current_height != 0 {
                self.adapter
                    .broadcast_height(Context::new(), current_height)
                    .await?;
            }
            Delay::new(Duration::from_millis(POLLING_BROADCAST)).await;
        }
    }

    async fn start_sync(
        &self,
        ctx: Context,
        sync_status_agent: StatusAgent,
        current_height: u64,
        remote_height: u64,
    ) -> ProtocolResult<()> {
        let mut current_height = current_height;

        loop {
            let current_block = self
                .adapter
                .get_block_by_height(ctx.clone(), current_height)
                .await?;

            let next_height = current_height + 1;

            let next_rich_block = self
                .get_rich_block_from_remote(ctx.clone(), next_height)
                .await?;

            self.verify_block(&current_block, &next_rich_block.block)?;

            self.commit_block(ctx.clone(), next_rich_block, sync_status_agent.clone())
                .await?;

            current_height = next_height;

            if current_height >= remote_height {
                return Ok(());
            }
        }
    }

    // TODO(yejiayu):
    // - Verify the proof
    // - Verify the block header
    // - Verify the transaction list
    fn verify_block(&self, current_block: &Block, next_block: &Block) -> ProtocolResult<()> {
        let block_hash = Hash::digest(current_block.encode_fixed()?);

        if block_hash != next_block.header.pre_hash {
            return Err(ConsensusError::SyncBlockHashErr(next_block.header.height).into());
        }
        Ok(())
    }

    async fn commit_block(
        &self,
        ctx: Context,
        rich_block: RichBlock,
        status_agent: StatusAgent,
    ) -> ProtocolResult<()> {
        let executor_resp = self
            .exec_block(ctx.clone(), rich_block.clone(), status_agent.clone())
            .await?;

        let block = &rich_block.block;
        let block_hash = Hash::digest(block.encode_fixed()?);

        let metadata = self.adapter.get_metadata(
            ctx.clone(),
            block.header.state_root.clone(),
            block.header.height,
            block.header.timestamp,
        )?;

        self.adapter.set_args(
            ctx.clone(),
            metadata.timeout_gap,
            metadata.cycles_limit,
            metadata.max_tx_size,
        );

        status_agent.update_by_commited(
            metadata,
            block.clone(),
            block_hash,
            // TODO(@yejiayu): need to get proof for the next block
            block.header.proof.clone(),
        );

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
                .get_txs_from_remote(ctx.clone(), &tx_hashes)
                .await?;
            for tx in remote_txs.into_iter() {
                txs.push(tx);
            }
        }

        Ok(RichBlock { block, txs })
    }

    async fn get_block_from_remote(&self, ctx: Context, height: u64) -> ProtocolResult<Block> {
        self.adapter
            .get_block_from_remote(ctx.clone(), height)
            .await
    }

    async fn save_chain_data(
        &self,
        ctx: Context,
        txs: Vec<SignedTransaction>,
        receipts: Vec<Receipt>,
        block: Block,
    ) -> ProtocolResult<()> {
        self.adapter.save_signed_txs(ctx.clone(), txs).await?;
        self.adapter.save_receipts(ctx.clone(), receipts).await?;
        self.adapter
            .save_proof(ctx.clone(), block.header.proof.clone())
            .await?;
        self.adapter.save_block(ctx.clone(), block).await?;
        Ok(())
    }

    async fn exec_block(
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
        };
        let resp = self.adapter.sync_exec(ctx, &exec_params, &rich_block.txs)?;

        status_agent.update_by_executed(ExecutedInfo::new(
            rich_block.block.header.height,
            rich_block.block.header.order_root,
            resp.clone(),
        ));

        Ok(resp)
    }

    async fn init_status_agent(&self) -> ProtocolResult<StatusAgent> {
        loop {
            let current_status = self.status.to_inner();

            if current_status.exec_height != current_status.current_height {
                Delay::new(Duration::from_millis(WAIT_EXECUTION)).await;
            } else {
                break;
            }
        }
        let current_status = self.status.to_inner();
        Ok(StatusAgent::new(current_status))
    }

    async fn need_sync(&self, ctx: Context, remote_height: u64) -> ProtocolResult<bool> {
        let mut current_height = self.status.to_inner().current_height;
        if remote_height == 0 {
            return Ok(false);
        }

        if remote_height <= current_height {
            return Ok(false);
        }

        if current_height == remote_height - 1 {
            let status = self.status.to_inner();
            Delay::new(Duration::from_millis(status.consensus_interval)).await;

            current_height = self.status.to_inner().current_height;
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
}
