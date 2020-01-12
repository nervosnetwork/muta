use std::sync::Arc;

use async_trait::async_trait;
use futures::lock::Mutex;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    Context, ExecutorParams, ExecutorResp, Synchronization, SynchronizationAdapter,
};
use protocol::types::{Epoch, Hash, Receipt, SignedTransaction};
use protocol::ProtocolResult;

use crate::status::{CurrentConsensusStatus, StatusAgent, UpdateInfo};
use crate::ConsensusError;

#[derive(Clone, Debug)]
pub struct RichEpoch {
    pub epoch: Epoch,
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
        let mut current_epoch_id = self.adapter.get_current_epoch_id(ctx.clone()).await?;
        if current_epoch_id >= remote_epoch_id - 1 {
            return Ok(());
        }

        // Lock the consensus engine, block commit process.
        let commit_lock = self.lock.try_lock();
        if commit_lock.is_none() {
            return Ok(());
        }

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

            self.verify_epoch(&current_epoch, &next_rich_epoch.epoch)?;
            self.commit_epoch(ctx.clone(), next_rich_epoch, sync_status_agent.clone())
                .await?;

            self.adapter
                .broadcast_epoch_id(ctx.clone(), current_epoch_id)
                .await?;
            current_epoch_id = next_epoch_id;

            if current_epoch_id >= remote_epoch_id {
                self.status.replace(sync_status_agent.to_inner());
                break;
            }
        }

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

    // TODO(yejiayu):
    // - Verify the proof
    // - Verify the epoch header
    // - Verify the transaction list
    fn verify_epoch(&self, current_epoch: &Epoch, next_epoch: &Epoch) -> ProtocolResult<()> {
        let epoch_hash = Hash::digest(current_epoch.encode_fixed()?);

        if epoch_hash != next_epoch.header.pre_hash {
            return Err(ConsensusError::SyncEpochHashErr(next_epoch.header.epoch_id).into());
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

        let epoch = &rich_epoch.epoch;
        let epoch_hash = Hash::digest(epoch.encode_fixed()?);
        status_agent.update_after_sync_commit(
            epoch.header.epoch_id,
            epoch.clone(),
            epoch_hash,
            epoch.header.proof.clone(),
        );

        self.save_chain_data(
            ctx.clone(),
            rich_epoch.txs.clone(),
            executor_resp.receipts.clone(),
            rich_epoch.epoch.clone(),
        )
        .await?;
        Ok(())
    }

    async fn get_rich_epoch_from_remote(
        &self,
        ctx: Context,
        epoch_id: u64,
    ) -> ProtocolResult<RichEpoch> {
        let epoch = self
            .adapter
            .get_epoch_from_remote(ctx.clone(), epoch_id)
            .await?;
        let txs = self
            .adapter
            .get_txs_from_remote(ctx, &epoch.ordered_tx_hashes)
            .await?;

        Ok(RichEpoch { epoch, txs })
    }

    async fn save_chain_data(
        &self,
        ctx: Context,
        txs: Vec<SignedTransaction>,
        receipts: Vec<Receipt>,
        epoch: Epoch,
    ) -> ProtocolResult<()> {
        self.adapter.save_signed_txs(ctx.clone(), txs).await?;
        self.adapter.save_receipts(ctx.clone(), receipts).await?;
        self.adapter
            .save_proof(ctx.clone(), epoch.header.proof.clone())
            .await?;
        self.adapter.save_epoch(ctx.clone(), epoch).await?;
        Ok(())
    }

    async fn exec_epoch(
        &self,
        ctx: Context,
        rich_epoch: RichEpoch,
        status_agent: StatusAgent,
    ) -> ProtocolResult<ExecutorResp> {
        let current_status = self.status.to_inner();
        let cycles_limit = current_status.cycles_limit;

        let exec_params = ExecutorParams {
            state_root: current_status.latest_state_root.clone(),
            epoch_id: rich_epoch.epoch.header.epoch_id,
            timestamp: rich_epoch.epoch.header.timestamp,
            cycles_limit,
        };
        let resp = self
            .adapter
            .sync_exec(ctx.clone(), &exec_params, &rich_epoch.txs)?;

        status_agent.update_after_exec(UpdateInfo::with_after_exec(
            rich_epoch.epoch.header.epoch_id,
            rich_epoch.epoch.header.order_root.clone(),
            resp.clone(),
        ));

        // If there are transactions in the trasnaction pool that have been on chain
        // after this execution, make sure they are cleaned up.
        self.adapter
            .flush_mempool(ctx.clone(), &rich_epoch.epoch.ordered_tx_hashes)
            .await?;
        Ok(resp)
    }

    async fn get_rich_epoch_from_local(
        &self,
        ctx: Context,
        epoch_id: u64,
    ) -> ProtocolResult<RichEpoch> {
        let epoch = self.adapter.get_epoch_by_id(ctx.clone(), epoch_id).await?;
        let txs = self
            .adapter
            .get_txs_from_storage(ctx.clone(), &epoch.ordered_tx_hashes)
            .await?;

        Ok(RichEpoch { epoch, txs })
    }

    async fn init_status_agent(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<StatusAgent> {
        let epoch = self.adapter.get_epoch_by_id(ctx.clone(), epoch_id).await?;
        let current_status = self.status.to_inner();

        let status = CurrentConsensusStatus {
            cycles_price:       current_status.cycles_price,
            cycles_limit:       current_status.cycles_limit,
            validators:         current_status.validators.clone(),
            consensus_interval: current_status.consensus_interval,
            prev_hash:          epoch.header.pre_hash.clone(),
            epoch_id:           epoch.header.epoch_id,
            exec_epoch_id:      epoch.header.exec_epoch_id,
            latest_state_root:  epoch.header.state_root.clone(),
            logs_bloom:         vec![],
            confirm_root:       vec![],
            receipt_root:       vec![],
            cycles_used:        vec![],
            state_root:         vec![],
            proof:              epoch.header.proof,
        };

        let status_agent = StatusAgent::new(status);
        let exec_epoch_id = epoch.header.exec_epoch_id;

        // Discard previous execution results and re-execute.
        if epoch_id != 0 {
            let exec_gap = epoch_id - exec_epoch_id;

            for gap in 1..=exec_gap {
                let rich_epoch = self
                    .get_rich_epoch_from_local(ctx.clone(), exec_epoch_id + gap)
                    .await?;
                self.exec_epoch(ctx.clone(), rich_epoch, status_agent.clone())
                    .await?;
            }
        }

        Ok(status_agent)
    }
}
