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

use crate::status::{CurrentConsensusStatus, StatusAgent, UpdateInfo};
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
}

#[async_trait]
impl<Adapter: SynchronizationAdapter> Synchronization for OverlordSynchronization<Adapter> {
    async fn receive_remote_block(&self, ctx: Context, remote_height: u64) -> ProtocolResult<()> {
        let block = self
            .get_block_from_remote(ctx.clone(), remote_height)
            .await?;

        if block.header.height != remote_height {
            log::error!("[synchronization]: block that doesn't match is found");
            return Ok(());
        }

        let current_height = self.adapter.get_current_height(Context::new()).await?;
        if remote_height == 0 || current_height >= remote_height - 1 {
            return Ok(());
        }
        // Lock the consensus engine, block commit process.
        let commit_lock = self.lock.try_lock();
        if commit_lock.is_none() {
            return Ok(());
        }

        log::info!(
            "[synchronization]: start, remote block height {:?} current block height {:?}",
            remote_height,
            current_height,
        );

        let sync_status_agent = self.init_status_agent(ctx.clone(), current_height).await?;
        let sync_resp = self
            .start_sync(
                ctx.clone(),
                sync_status_agent.clone(),
                current_height,
                remote_height,
            )
            .await;
        let mut sync_status = sync_status_agent.to_inner();
        let current_height = sync_status.height;

        if let Err(e) = sync_resp {
            log::error!(
                "[synchronization]: err, current_height {:?} err_msg: {:?}",
                current_height,
                e
            );
        }

        sync_status.height += 1;
        self.status.replace(sync_status.clone());
        self.adapter.update_status(
            ctx.clone(),
            sync_status.height,
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
    pub fn new(adapter: Arc<Adapter>, status: StatusAgent, lock: Arc<Mutex<()>>) -> Self {
        Self {
            adapter,
            status,
            lock,
        }
    }

    pub async fn polling_broadcast(&self) -> ProtocolResult<()> {
        loop {
            let current_height = self.adapter.get_current_height(Context::new()).await?;
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

        self.adapter
            .set_timeout_gap(ctx.clone(), metadata.timeout_gap);

        status_agent.update_after_sync_commit(
            block.header.height,
            metadata,
            block.clone(),
            block_hash,
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
        let txs = self
            .adapter
            .get_txs_from_remote(ctx, &block.ordered_tx_hashes)
            .await?;

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
            state_root: current_status.latest_state_root.clone(),
            height: rich_block.block.header.height,
            timestamp: rich_block.block.header.timestamp,
            cycles_limit,
        };
        let resp = self
            .adapter
            .sync_exec(ctx.clone(), &exec_params, &rich_block.txs)?;

        status_agent.update_after_exec(UpdateInfo::with_after_exec(
            rich_block.block.header.height,
            rich_block.block.header.order_root.clone(),
            resp.clone(),
        ));

        Ok(resp)
    }

    async fn get_rich_block_from_local(
        &self,
        ctx: Context,
        height: u64,
    ) -> ProtocolResult<RichBlock> {
        let block = self
            .adapter
            .get_block_by_height(ctx.clone(), height)
            .await?;
        let txs = self
            .adapter
            .get_txs_from_storage(ctx.clone(), &block.ordered_tx_hashes)
            .await?;

        Ok(RichBlock { block, txs })
    }

    async fn init_status_agent(&self, ctx: Context, height: u64) -> ProtocolResult<StatusAgent> {
        loop {
            let current_status = self.status.to_inner();

            if current_status.exec_height != current_status.height - 1 {
                Delay::new(Duration::from_millis(WAIT_EXECUTION)).await;
            } else {
                break;
            }
        }

        let block = self
            .adapter
            .get_block_by_height(ctx.clone(), height)
            .await?;
        let current_status = self.status.to_inner();

        let status = CurrentConsensusStatus {
            cycles_price:       current_status.cycles_price,
            cycles_limit:       current_status.cycles_limit,
            validators:         current_status.validators.clone(),
            consensus_interval: current_status.consensus_interval,
            propose_ratio:      current_status.propose_ratio,
            prevote_ratio:      current_status.prevote_ratio,
            precommit_ratio:    current_status.precommit_ratio,
            brake_ratio:        current_status.brake_ratio,
            prev_hash:          block.header.pre_hash.clone(),
            height:             block.header.height,
            exec_height:        block.header.exec_height,
            latest_state_root:  block.header.state_root.clone(),
            logs_bloom:         vec![],
            confirm_root:       vec![],
            receipt_root:       vec![],
            cycles_used:        vec![],
            state_root:         vec![],
            proof:              block.header.proof,
        };

        let status_agent = StatusAgent::new(status);
        let exec_height = block.header.exec_height;

        // Discard previous execution results and re-execute.
        if height != 0 {
            let exec_gap = height - exec_height;

            for gap in 1..=exec_gap {
                let rich_block = self
                    .get_rich_block_from_local(ctx.clone(), exec_height + gap)
                    .await?;
                self.exec_block(ctx.clone(), rich_block, status_agent.clone())
                    .await?;
            }
        }

        Ok(status_agent)
    }
}
