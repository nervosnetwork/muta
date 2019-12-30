use std::sync::Arc;
use std::time::Duration;

use creep::Context;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::stream::StreamExt;
use futures_timer::Delay;
use log::{error, warn};
use overlord::types::{OverlordMsg, Status};
use overlord::OverlordHandler;

use protocol::fixed_codec::ProtocolFixedCodec;
use protocol::traits::ConsensusAdapter;
use protocol::types::{Address, Hash, Proof};
use protocol::ProtocolResult;

use crate::engine::ConsensusEngine;
use crate::fixed_types::FixedPill;
use crate::ConsensusError;

const POLL_AWAIT_TIME: u64 = 50;

pub struct Synchronization<Adapter: ConsensusAdapter + 'static> {
    rich_id_rx: UnboundedReceiver<(u64, Context)>,
    handler:    OverlordHandler<FixedPill>,
    engine:     Arc<ConsensusEngine<Adapter>>,
}

impl<Adapter> Synchronization<Adapter>
where
    Adapter: ConsensusAdapter + 'static,
{
    pub fn new(
        handler: OverlordHandler<FixedPill>,
        engine: Arc<ConsensusEngine<Adapter>>,
    ) -> (Self, UnboundedSender<(u64, Context)>) {
        let (tx, rich_id_rx) = unbounded();
        let sync = Synchronization {
            engine,
            handler,
            rich_id_rx,
        };
        (sync, tx)
    }

    pub async fn run(mut self) {
        loop {
            if let Some((id, ctx)) = self.rich_id_rx.next().await {
                if let Err(e) = self.process(id - 1, ctx).await {
                    error!("synchronization error {:?}", e);
                }
            }
        }
    }

    async fn process(&mut self, rich_epoch_id: u64, ctx: Context) -> ProtocolResult<()> {
        let current_epoch_id = self.get_current_epoch_id(ctx.clone()).await;
        if current_epoch_id >= rich_epoch_id - 1 {
            return Ok(());
        }

        // Lock the consensus engine, block commit process.
        let commit_lock = self.engine.lock.try_lock();
        if commit_lock.is_none() {
            return Ok(());
        }

        error!("self {}, chain {}", current_epoch_id, rich_epoch_id);
        error!("consensus: start synchronization");

        let mut current_hash = self.get_prev_hash().await?;

        for id in (current_epoch_id + 1)..=rich_epoch_id {
            error!("consensus: start synchronization epoch {}", id);

            // First pull a new block.
            warn!("consensus: synchronization pull epoch {}", id);
            let epoch = self.engine.pull_epoch(ctx.clone(), id).await?;
            // Check proof and previous hash.
            warn!("consensus: synchronization check proof and previous hash");
            let proof = epoch.header.proof.clone();
            self.check_proof(id, proof.clone())?;

            if id != 1 && current_hash != epoch.header.pre_hash {
                return Err(ConsensusError::SyncEpochHashErr(id).into());
            }
            if !self.engine.check_state_root(&epoch.header.state_root) {
                return Err(ConsensusError::SyncEpochStateRootErr(id).into());
            }

            self.engine.save_proof(ctx.clone(), proof.clone()).await?;

            // Then pull signed transactions.
            warn!("consensus: synchronization pull signed transactions");
            let ordered_tx_hashes = epoch.ordered_tx_hashes.clone();
            let txs = self
                .engine
                .pull_txs(ctx.clone(), ordered_tx_hashes.clone())
                .await?;

            warn!("consensus: synchronization executor the epoch");
            self.engine
                .exec(
                    epoch.header.order_root.clone(),
                    epoch.header.epoch_id,
                    Address::User(epoch.header.proposer.clone()),
                    txs.clone(),
                )
                .await?;

            self.engine
                .update_status(id, epoch.clone(), epoch.header.proof.clone(), txs)
                .await?;

            current_hash = Hash::digest(epoch.encode_fixed()?);
            self.achieve_exec(id).await;
        }

        self.transmit_rich_status(ctx, rich_epoch_id).await
    }

    async fn transmit_rich_status(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<()> {
        warn!(
            "consensus: synchronization send overlord rich status {}",
            epoch_id
        );

        let status = Status {
            epoch_id:       epoch_id + 1,
            interval:       Some(self.engine.get_current_interval()),
            authority_list: self.engine.get_current_authority_list(),
        };

        self.handler
            .send_msg(ctx, OverlordMsg::RichStatus(status))
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;

        Ok(())
    }

    async fn achieve_exec(&self, epoch_id: u64) {
        loop {
            if epoch_id == self.engine.get_exec_epoch_id() {
                return;
            }
            Delay::new(Duration::from_millis(POLL_AWAIT_TIME)).await;
        }
    }

    async fn get_current_epoch_id(&self, ctx: Context) -> u64 {
        self.engine
            .get_current_epoch_id(ctx)
            .await
            .expect("No epoch in DB")
    }

    async fn get_prev_hash(&self) -> ProtocolResult<Hash> {
        // let mut state_root = Hash::from_empty();
        // let current_hash = if current_epoch_id != 0 {
        //     let current_epoch = self
        //         .engine
        //         .get_epoch_by_id(ctx.clone(), current_epoch_id)
        //         .await?;
        //     state_root = current_epoch.header.state_root.clone();
        //     let tmp = Hash::digest(current_epoch.encode_fixed()?);

        //     // Check epoch for the first time.
        //     let epoch_header = self
        //         .engine
        //         .pull_epoch(ctx.clone(), current_epoch_id + 1)
        //         .await?
        //         .header;
        //     self.check_proof(current_epoch_id + 1, epoch_header.proof.clone())?;
        //     if tmp != epoch_header.pre_hash {
        //         return Err(ConsensusError::SyncEpochHashErr(current_epoch_id +
        // 1).into());     }
        //     tmp
        // } else {
        //     Hash::from_empty()
        // };
        let current_hash = self.engine.get_current_prev_hash();
        Ok(current_hash)
    }

    fn check_proof(&self, _epoch_id: u64, _proof: Proof) -> ProtocolResult<()> {
        Ok(())
    }
}
