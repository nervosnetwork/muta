use std::convert::TryInto;
use std::error::Error;
use std::sync::Arc;

use futures03::compat::Future01CompatExt;
use futures03::prelude::{FutureExt, TryFutureExt};
use futures_locks::Mutex;
use log::error;
use uuid::Uuid;

use core_consensus::{Consensus, Engine, Synchronizer};
use core_context::Context;
use core_crypto::Crypto;
use core_runtime::{Executor, TransactionPool};
use core_serialization as ser;
use core_storage::Storage;
use core_types::Block;

use crate::p2p::message::synchronizer::{
    packed_message, BroadcastStatus, PullBlocks, PushBlocks, SynchronizerMessage,
};
use crate::p2p::{self as p2p, Broadcaster};
use crate::reactor::{CallbackMap, Reaction, Reactor, ReactorMessage};

const SYNC_STEP: u64 = 20;

pub struct SynchronizerReactor<E, T, S, C, Y, Co>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    Y: Synchronizer + 'static,
    Co: Consensus + 'static,
{
    engine:         Arc<Engine<E, T, S, C>>,
    storage:        Arc<S>,
    synchronizer:   Arc<Y>,
    consensus:      Arc<Co>,
    callback_map:   CallbackMap,
    current_height: Mutex<u64>,
}

impl<E, T, S, C, Y, Co> Reactor for SynchronizerReactor<E, T, S, C, Y, Co>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    Y: Synchronizer + 'static,
    Co: Consensus + 'static,
{
    type Input = (Context, SynchronizerMessage);
    type Output = Reaction<ReactorMessage>;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        if let (ctx, SynchronizerMessage { message: Some(msg) }) = input {
            match msg {
                packed_message::Message::BroadcastStatus(status) => {
                    let fut = Self::update_status(
                        ctx.clone(),
                        self.current_height.clone(),
                        Arc::clone(&self.engine),
                        Arc::clone(&self.storage),
                        Arc::clone(&self.synchronizer),
                        Arc::clone(&self.consensus),
                        status,
                    );
                    Reaction::Done(Box::new(fut.unit_error().boxed().compat()))
                }
                packed_message::Message::PullBlocks(pull_msg) => {
                    let fut = Self::pull_blocks(
                        ctx.clone(),
                        Arc::clone(&self.storage),
                        broadcaster.clone(),
                        pull_msg,
                    );
                    Reaction::Done(Box::new(fut.unit_error().boxed().compat()))
                }
                packed_message::Message::PushBlocks(push_msg) => {
                    let fut =
                        Self::push_blocks(ctx.clone(), Arc::clone(&self.callback_map), push_msg);
                    Reaction::Done(Box::new(fut.unit_error().boxed().compat()))
                }
            }
        } else {
            unreachable!()
        }
    }
}

impl<E, T, S, C, Y, Co> SynchronizerReactor<E, T, S, C, Y, Co>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    Y: Synchronizer + 'static,
    Co: Consensus + 'static,
{
    pub fn new(
        storage: Arc<S>,
        engine: Arc<Engine<E, T, S, C>>,
        synchronizer: Arc<Y>,
        consensus: Arc<Co>,
        callback_map: CallbackMap,
    ) -> Self {
        Self {
            storage,
            engine,
            synchronizer,
            consensus,
            callback_map,
            current_height: Mutex::new(0),
        }
    }

    async fn pull_blocks(
        ctx: Context,
        storage: Arc<S>,
        broadcaster: Broadcaster,
        pull_msg: PullBlocks,
    ) {
        let res = await!(Self::pull_blocks_with_err(
            ctx,
            storage,
            broadcaster,
            pull_msg
        ));
        if let Err(e) = res {
            error!("pull_blocks err: {:?}", e);
        }
    }

    async fn pull_blocks_with_err(
        ctx: Context,
        storage: Arc<S>,
        mut broadcaster: Broadcaster,
        pull_msg: PullBlocks,
    ) -> Result<(), Box<dyn Error>> {
        let PullBlocks { uuid, heights } = pull_msg;
        let mut blocks = vec![];
        let latest_proof = await!(storage.get_latest_proof(ctx.clone()).compat())?;
        let current_height = latest_proof.height;
        for height in heights
            .into_iter()
            .filter(|height| *height <= current_height)
        {
            let block = await!(storage.get_block_by_height(ctx.clone(), height).compat())?;
            blocks.push(block);
            if height == current_height {
                let mut proof_block = Block::default();
                proof_block.header.height = ::std::u64::MAX;
                proof_block.header.proof = latest_proof.clone();
                blocks.push(proof_block);
            }
        }
        let pb_blocks: Vec<ser::Block> = blocks.into_iter().map(std::convert::Into::into).collect();
        broadcaster.send(
            ctx,
            p2p::message::Message::SynchronizerMessage(SynchronizerMessage::push_blocks(
                uuid, pb_blocks,
            )),
        );
        Ok(())
    }

    async fn push_blocks_with_err(
        _ctx: Context,
        callback_map: CallbackMap,
        push_msg: PushBlocks,
    ) -> Result<(), Box<dyn Error>> {
        let PushBlocks {
            uuid,
            blocks: ser_blocks,
        } = push_msg;

        if let Ok(uuid) = Uuid::parse_str(uuid.as_str()) {
            if let Some(mut done_tx) = callback_map.write().remove(&uuid) {
                let mut blocks = vec![];
                for ser_block in ser_blocks {
                    let block = TryInto::<Block>::try_into(ser_block)?;
                    blocks.push(block);
                }
                done_tx.try_send(Box::new(blocks))?;
            }
        }
        Ok(())
    }

    async fn push_blocks(ctx: Context, callback_map: CallbackMap, push_msg: PushBlocks) {
        let res = await!(Self::push_blocks_with_err(ctx, callback_map, push_msg));
        if let Err(e) = res {
            error!("push_blocks err: {:?}", e);
        }
    }

    async fn update_status(
        ctx: Context,
        current_height: Mutex<u64>,
        engine: Arc<Engine<E, T, S, C>>,
        storage: Arc<S>,
        synchronizer: Arc<Y>,
        consensus: Arc<Co>,
        status: BroadcastStatus,
    ) {
        let res = await!(Self::update_status_with_err(
            ctx,
            current_height,
            engine,
            storage,
            synchronizer,
            consensus,
            status
        ));
        if let Err(e) = res {
            error!("update_status err: {:?}", e);
        }
    }

    async fn update_status_with_err(
        ctx: Context,
        mut current_height_atomic: Mutex<u64>,
        engine: Arc<Engine<E, T, S, C>>,
        storage: Arc<S>,
        synchronizer: Arc<Y>,
        consensus: Arc<Co>,
        status: BroadcastStatus,
    ) -> Result<(), Box<dyn Error>> {
        let current_height_option = current_height_atomic.get_mut();
        if current_height_option.is_none() {
            return Ok(());
        }
        let current_height = current_height_option.unwrap();

        let global_height = status.height;
        if global_height <= *current_height {
            return Ok(());
        }
        let latest_block: Block = await!(storage.get_latest_block(ctx.clone()).compat())?;
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
            let mut blocks = await!(synchronizer.pull_blocks(ctx.clone(), heights).compat())?;
            all_blocks.append(&mut blocks);
            let last_index = all_blocks.len() - 1;
            for i in 0..last_index {
                let block = all_blocks[i].clone();
                let proof = all_blocks[i + 1].header.proof.clone();
                await!(engine.insert_sync_block(ctx.clone(), block, proof))?;
            }
            all_blocks = all_blocks.split_off(last_index);
        }

        // send status after synchronizing blocks, trigger bft
        await!(consensus.send_status().compat())?;

        Ok(())
    }
}
