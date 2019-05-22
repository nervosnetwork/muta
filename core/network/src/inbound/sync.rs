use std::clone::Clone;
use std::sync::Arc;

use futures::compat::Future01CompatExt;
use futures::lock::Mutex;
use log::error;

use common_channel::Sender;
use core_context::Context;
use core_network_message::common::{PullTxs, PushTxs};
use core_network_message::sync::{BroadcastStatus, PullBlocks, PushBlocks};
use core_network_message::{Codec, Method};
use core_runtime::{network::Synchronizer, Consensus, Storage};
use core_types::{Block, SignedTransaction};

use crate::common::scope_from_context;
use crate::{BytesBroadcaster, CallbackMap, Error, OutboundHandle};

const SYNC_STEP: u64 = 20;

pub struct SyncReactor<C, S> {
    consensus: Arc<C>,
    storage:   Arc<S>,

    callback:       Arc<CallbackMap>,
    outbound:       OutboundHandle,
    current_height: Arc<Mutex<u64>>,
}

impl<C, S> Clone for SyncReactor<C, S> {
    fn clone(&self) -> Self {
        SyncReactor {
            consensus: Arc::clone(&self.consensus),
            storage:   Arc::clone(&self.storage),

            callback:       Arc::clone(&self.callback),
            outbound:       self.outbound.clone(),
            current_height: Arc::clone(&self.current_height),
        }
    }
}

impl<C, S> SyncReactor<C, S>
where
    C: Consensus + 'static,
    S: Storage + 'static,
{
    pub fn new(
        consensus: Arc<C>,
        storage: Arc<S>,
        callback: Arc<CallbackMap>,
        outbound: OutboundHandle,
    ) -> Self {
        SyncReactor {
            consensus,
            storage,

            callback,
            outbound,
            current_height: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> Result<(), Error> {
        match method {
            Method::SyncBroadcastStatus => self.handle_broadcast_status(ctx, data).await?,
            Method::SyncPullBlocks => self.handle_pull_blocks(ctx, data).await?,
            Method::SyncPushBlocks => self.handle_push_blocks(ctx, data).await?,
            Method::SyncPullTxs => self.handle_pull_txs(ctx, data).await?,
            Method::SyncPushTxs => self.handle_push_txs(ctx, data).await?,
            _ => Err(Error::UnknownMethod(method.to_u32()))?,
        };

        Ok(())
    }

    pub async fn handle_broadcast_status(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let status = <BroadcastStatus as Codec>::decode(data.as_slice())?;
        let mut current_height = self.current_height.lock().await;

        let global_height = status.height;
        if global_height <= *current_height {
            return Ok(());
        }

        let latest_block: Block = self.storage.get_latest_block(ctx.clone()).compat().await?;
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
            let mut blocks = self
                .outbound
                .pull_blocks(ctx.clone(), heights)
                .compat()
                .await?;
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
                        .pull_txs_sync(ctx.clone(), &block.tx_hashes)
                        .compat()
                        .await?
                };
                self.consensus
                    .insert_sync_block(ctx.clone(), block, signed_txs, proof)
                    .compat()
                    .await?;
            }
            all_blocks = all_blocks.split_off(last_index);
        }

        // send status after synchronizing blocks, trigger bft
        self.consensus.send_status().compat().await?;

        Ok(())
    }

    pub async fn handle_pull_blocks(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let PullBlocks { uid, heights } = <PullBlocks as Codec>::decode(data.as_slice())?;

        let mut blocks = vec![];
        let latest_proof = self.storage.get_latest_proof(ctx.clone()).compat().await?;
        let current_height = latest_proof.height;
        for height in heights
            .into_iter()
            .filter(|height| *height <= current_height)
        {
            let block = self
                .storage
                .get_block_by_height(ctx.clone(), height)
                .compat()
                .await?;

            blocks.push(block);
            if height == current_height {
                let mut proof_block = Block::default();
                proof_block.header.height = ::std::u64::MAX;
                proof_block.header.proof = latest_proof.clone();
                blocks.push(proof_block);
            }
        }

        let push_blocks = PushBlocks::from(uid, blocks);
        let scope = scope_from_context(ctx).ok_or(Error::SessionIdNotFound)?;
        if let Err(err) =
            self.outbound
                .quick_filter_broadcast(Method::SyncPushBlocks, push_blocks, scope)
        {
            error!("net [inbound]: push_blocks: [err: {:?}]", err);
        }

        Ok(())
    }

    pub async fn handle_push_blocks(&self, _: Context, data: Vec<u8>) -> Result<(), Error> {
        let push_blocks = <PushBlocks as Codec>::decode(data.as_slice())?;
        let uid = push_blocks.uid;

        let done_tx = self
            .callback
            .take::<Sender<Vec<Block>>>(uid)
            .ok_or_else(|| Error::CallbackItemNotFound(uid))?;
        let blocks = push_blocks.des()?;

        done_tx
            .try_send(blocks)
            .map_err(|_| Error::CallbackTrySendError)?;

        Ok(())
    }

    pub async fn handle_pull_txs(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let pull_txs = <PullTxs as Codec>::decode(data.as_slice())?;
        let uid = pull_txs.uid;
        let hashes = pull_txs.des()?;

        let stxs = self
            .storage
            .get_transactions(ctx.clone(), hashes.as_slice())
            .compat()
            .await?;

        let push_txs = PushTxs::from(uid, stxs);
        let scope = scope_from_context(ctx).ok_or(Error::SessionIdNotFound)?;
        if let Err(err) = self
            .outbound
            .quick_filter_broadcast(Method::SyncPushTxs, push_txs, scope)
        {
            error!("net [inbound]: push_txs: [err: {:?}]", err);
        }

        Ok(())
    }

    pub async fn handle_push_txs(&self, _: Context, data: Vec<u8>) -> Result<(), Error> {
        let push_txs = <PushTxs as Codec>::decode(data.as_slice())?;
        let uid = push_txs.uid;

        let done_tx = self
            .callback
            .take::<Sender<Vec<SignedTransaction>>>(uid)
            .ok_or_else(|| Error::CallbackItemNotFound(uid))?;
        let stxs = push_txs.des()?;

        done_tx
            .try_send(stxs)
            .map_err(|_| Error::CallbackTrySendError)?;

        Ok(())
    }
}
