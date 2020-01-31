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

#[derive(Clone, Debug)]
pub struct RichEpoch {
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
    async fn receive_remote_epoch(&self, ctx: Context, remote_epoch_id: u64) -> ProtocolResult<()> {
        let mut current_epoch_id = self.adapter.get_current_epoch_id(Context::new()).await?;
        if remote_epoch_id == 0 || current_epoch_id >= remote_epoch_id - 1 {
            return Ok(());
        }
        // Lock the consensus engine, block commit process.
        let commit_lock = self.lock.try_lock();
        if commit_lock.is_none() {
            return Ok(());
        }

        log::info!(
            "[synchronization]: start, remote block id {:?} current block id {:?}",
            remote_epoch_id,
            current_epoch_id,
        );

        let sync_status_agent = self
            .init_status_agent(ctx.clone(), current_epoch_id)
            .await?;

        loop {
            let current_epoch = self
                .adapter
                .get_epoch_by_id(ctx.clone(), current_epoch_id)
                .await?;

            let next_epoch_id = current_epoch_id + 1;

            let next_rich_epoch = self
                .get_rich_epoch_from_remote(ctx.clone(), next_epoch_id)
                .await?;

            self.verify_epoch(&current_epoch, &next_rich_epoch.block)?;

            self.commit_epoch(ctx.clone(), next_rich_epoch, sync_status_agent.clone())
                .await?;

            self.adapter
                .broadcast_epoch_id(ctx.clone(), current_epoch_id)
                .await?;

            current_epoch_id = next_epoch_id;

            if current_epoch_id >= remote_epoch_id {
                let mut sync_status = sync_status_agent.to_inner();
                sync_status.height += 1;
                self.status.replace(sync_status.clone());
                self.adapter.update_status(
                    ctx.clone(),
                    sync_status.height,
                    sync_status.consensus_interval,
                    sync_status.validators,
                )?;
                break;
            }
        }

        log::info!(
            "[synchronization] end, remote block id {:?} current block id {:?}",
            remote_epoch_id,
            current_epoch_id,
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
            let current_epoch_id = self.adapter.get_current_epoch_id(Context::new()).await?;
            if current_epoch_id != 0 {
                self.adapter
                    .broadcast_epoch_id(Context::new(), current_epoch_id)
                    .await?;
            }
            Delay::new(Duration::from_millis(POLLING_BROADCAST)).await;
        }
    }

    // TODO(yejiayu):
    // - Verify the proof
    // - Verify the block header
    // - Verify the transaction list
    fn verify_epoch(&self, current_epoch: &Block, next_epoch: &Block) -> ProtocolResult<()> {
        let epoch_hash = Hash::digest(current_epoch.encode_fixed()?);

        if epoch_hash != next_epoch.header.pre_hash {
            return Err(ConsensusError::SyncEpochHashErr(next_epoch.header.height).into());
        }
        Ok(())
    }

    async fn commit_epoch(
        &self,
        ctx: Context,
        rich_epoch: RichEpoch,
        status_agent: StatusAgent,
    ) -> ProtocolResult<()> {
        let executor_resp = self
            .exec_epoch(ctx.clone(), rich_epoch.clone(), status_agent.clone())
            .await?;

        let block = &rich_epoch.block;
        let epoch_hash = Hash::digest(block.encode_fixed()?);
        status_agent.update_after_sync_commit(
            block.header.height,
            block.clone(),
            epoch_hash,
            block.header.proof.clone(),
        );

        self.save_chain_data(
            ctx.clone(),
            rich_epoch.txs.clone(),
            executor_resp.receipts.clone(),
            rich_epoch.block.clone(),
        )
        .await?;
        Ok(())
    }

    async fn get_rich_epoch_from_remote(
        &self,
        ctx: Context,
        height: u64,
    ) -> ProtocolResult<RichEpoch> {
        let block = self
            .adapter
            .get_epoch_from_remote(ctx.clone(), height)
            .await?;
        let txs = self
            .adapter
            .get_txs_from_remote(ctx, &block.ordered_tx_hashes)
            .await?;

        Ok(RichEpoch { block, txs })
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
        self.adapter.save_epoch(ctx.clone(), block).await?;
        Ok(())
    }

    async fn exec_epoch(
        &self,
        ctx: Context,
        rich_epoch: RichEpoch,
        status_agent: StatusAgent,
    ) -> ProtocolResult<ExecutorResp> {
        let current_status = status_agent.to_inner();
        let cycles_limit = current_status.cycles_limit;

        let exec_params = ExecutorParams {
            state_root: current_status.latest_state_root.clone(),
            height: rich_epoch.block.header.height,
            timestamp: rich_epoch.block.header.timestamp,
            cycles_limit,
        };
        let resp = self
            .adapter
            .sync_exec(ctx.clone(), &exec_params, &rich_epoch.txs)?;

        status_agent.update_after_exec(UpdateInfo::with_after_exec(
            rich_epoch.block.header.height,
            rich_epoch.block.header.order_root.clone(),
            resp.clone(),
        ));

        // If there are transactions in the trasnaction pool that have been on chain
        // after this execution, make sure they are cleaned up.
        self.adapter
            .flush_mempool(ctx.clone(), &rich_epoch.block.ordered_tx_hashes)
            .await?;
        Ok(resp)
    }

    async fn get_rich_epoch_from_local(
        &self,
        ctx: Context,
        height: u64,
    ) -> ProtocolResult<RichEpoch> {
        let block = self.adapter.get_epoch_by_id(ctx.clone(), height).await?;
        let txs = self
            .adapter
            .get_txs_from_storage(ctx.clone(), &block.ordered_tx_hashes)
            .await?;

        Ok(RichEpoch { block, txs })
    }

    async fn init_status_agent(&self, ctx: Context, height: u64) -> ProtocolResult<StatusAgent> {
        let block = self.adapter.get_epoch_by_id(ctx.clone(), height).await?;
        let current_status = self.status.to_inner();

        let status = CurrentConsensusStatus {
            cycles_price:       current_status.cycles_price,
            cycles_limit:       current_status.cycles_limit,
            validators:         current_status.validators.clone(),
            consensus_interval: current_status.consensus_interval,
            prev_hash:          block.header.pre_hash.clone(),
            height:           block.header.height,
            exec_height:      block.header.exec_height,
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
                let rich_epoch = self
                    .get_rich_epoch_from_local(ctx.clone(), exec_height + gap)
                    .await?;
                self.exec_epoch(ctx.clone(), rich_epoch, status_agent.clone())
                    .await?;
            }
        }

        Ok(status_agent)
    }
}
