use std::sync::Arc;

use futures::lock::Mutex;

use core_context::Context;
use core_runtime::{network::Synchronizer as Network, Consensus, Storage};
use core_runtime::{FutSyncResult, Synchronization, SynchronizerError};
use core_types::{Block, Hash, SignedTransaction};

use crate::DefaultOutboundHandle;

const SYNC_STEP: u64 = 20;

pub type SyncResult<T> = Result<T, SynchronizerError>;

// TODO: move sync code to standalone core_sync crate
pub struct Synchronizer<C, S> {
    consensus: Arc<C>,
    storage:   Arc<S>,

    outbound:       DefaultOutboundHandle,
    current_height: Arc<Mutex<u64>>,
}

impl<C, S> Synchronizer<C, S> {
    pub fn new(consensus: Arc<C>, storage: Arc<S>, outbound: DefaultOutboundHandle) -> Self {
        let current_height = Arc::new(Mutex::new(0));

        Synchronizer {
            consensus,
            storage,
            outbound,

            current_height,
        }
    }
}

impl<C, S> Synchronizer<C, S>
where
    C: Consensus,
    S: Storage,
{
    #[allow(clippy::needless_lifetimes)]
    pub async fn sync_blocks(&self, ctx: Context, global_height: u64) -> SyncResult<()> {
        let current_height_result = self.current_height.try_lock();
        if current_height_result.is_none() {
            log::info!("fail to get lock in sync_blocks");
            return Ok(());
        }
        let mut current_height = current_height_result.unwrap();

        if global_height <= *current_height {
            return Ok(());
        }

        let latest_block: Block = self.storage.get_latest_block(ctx.clone()).await?;
        *current_height = latest_block.header.height;
        // ignore update process if height difference is less than or equal to 1,
        // wait for consensus to catch up
        if global_height <= *current_height + 1 {
            return Ok(());
        }

        // Every block's proof is in the block of next height, for the lastest block,
        // the proof will be put in a virtual block whose height is ::std::u64::MAX.
        // Use SYNC_STEP to avoid geting a bulk of blocks from peers at one time.
        let mut all_blocks = vec![];
        for height in ((*current_height + 1)..=global_height).step_by(SYNC_STEP as usize) {
            let heights = (height..(height + SYNC_STEP)).collect::<Vec<_>>();
            let mut blocks = self.outbound.pull_blocks(ctx.clone(), heights).await?;
            all_blocks.append(&mut blocks);

            let last_index = all_blocks.len() - 1;
            for i in 0..last_index {
                let block = all_blocks[i].clone();
                let proof = all_blocks[i + 1].header.proof.clone();
                let signed_txs = if block.tx_hashes.is_empty() {
                    vec![]
                } else {
                    // todo: if there are too many txes, split it into small request
                    self.outbound
                        .pull_txs(ctx.clone(), &block.tx_hashes)
                        .await?
                };

                self.consensus
                    .insert_sync_block(ctx.clone(), block, signed_txs, proof)
                    .await?;
            }
            all_blocks = all_blocks.split_off(last_index);
        }

        // send status after synchronizing blocks, trigger bft
        self.consensus.send_status().await?;

        Ok(())
    }

    pub async fn get_blocks(&self, ctx: Context, heights: Vec<u64>) -> SyncResult<Vec<Block>> {
        let storage = &self.storage;

        let mut blocks = Vec::with_capacity(heights.len() + 1);
        let latest_proof = storage.get_latest_proof(ctx.clone()).await?;
        let current_height = latest_proof.height;

        let avail_heights = heights.into_iter().filter(|h| *h <= current_height);
        for height in avail_heights {
            let block = storage.get_block_by_height(ctx.clone(), height).await?;
            blocks.push(block);

            if height == current_height {
                let mut proof_block = Block::default();
                proof_block.header.height = ::std::u64::MAX;
                proof_block.header.proof = latest_proof.clone();
                blocks.push(proof_block);
            }
        }

        Ok(blocks)
    }

    pub async fn get_stxs(
        &self,
        ctx: Context,
        hashes: Vec<Hash>,
    ) -> SyncResult<Vec<SignedTransaction>> {
        let storage = &self.storage;

        Ok(storage.get_transactions(ctx, hashes.as_slice()).await?)
    }
}

impl<C, S> Clone for Synchronizer<C, S> {
    fn clone(&self) -> Self {
        Synchronizer {
            consensus: Arc::clone(&self.consensus),
            storage:   Arc::clone(&self.storage),

            outbound:       self.outbound.clone(),
            current_height: Arc::clone(&self.current_height),
        }
    }
}

impl<C, S> Synchronization for Synchronizer<C, S>
where
    C: Consensus + 'static,
    S: Storage + 'static,
{
    fn sync_blocks(&self, ctx: Context, global_height: u64) -> FutSyncResult<()> {
        let synchronizer = self.clone();

        Box::pin(async move { synchronizer.sync_blocks(ctx, global_height).await })
    }

    fn get_blocks(&self, ctx: Context, heights: Vec<u64>) -> FutSyncResult<Vec<Block>> {
        let synchronizer = self.clone();

        Box::pin(async move { synchronizer.get_blocks(ctx, heights).await })
    }

    fn get_stxs(&self, ctx: Context, hashes: Vec<Hash>) -> FutSyncResult<Vec<SignedTransaction>> {
        let synchronizer = self.clone();

        Box::pin(async move { synchronizer.get_stxs(ctx, hashes).await })
    }
}
