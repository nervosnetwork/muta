use std::any::Any;
use std::marker::Send;

use futures::future::{err, ok};
use futures::sync::mpsc::channel;
use futures::sync::mpsc::Sender;
use futures03::{
    compat::Stream01CompatExt,
    prelude::{FutureExt, StreamExt, TryFutureExt},
};
use log::error;
use uuid::Uuid;

use core_consensus::{Status, Synchronizer, SynchronizerError};
use core_context::Context;
use core_runtime::FutRuntimeResult;
use core_types::{Block, Hash, SignedTransaction};

use crate::p2p::{Broadcaster, Message as P2PMessage};
use crate::reactor::outbound::{OutboundMessage, Sender as OutboundSender};
use crate::reactor::{CallbackMap, FutReactorResult, Reactor};
use crate::Message;

#[derive(Clone, Debug)]
pub enum SynchronizerMessage {
    BroadcastStatus {
        status: Status,
    },
    PullBlocks {
        ctx:     Context,
        done:    Sender<Box<dyn Any + Send + 'static>>,
        heights: Vec<u64>,
    },
    PullTxsSync {
        ctx:    Context,
        done:   Sender<Box<dyn Any + Send + 'static>>,
        hashes: Vec<Hash>,
    },
}

pub struct SynchronizerReactor {
    callback_map: CallbackMap,
}

impl SynchronizerReactor {
    pub fn new(callback_map: CallbackMap) -> Self {
        Self { callback_map }
    }
}

impl Reactor for SynchronizerReactor {
    type Input = SynchronizerMessage;
    type Output = FutReactorResult;

    fn react(&mut self, mut broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        match input {
            SynchronizerMessage::BroadcastStatus { status } => broadcaster.send(
                Context::new(),
                P2PMessage::from(Message::BroadcastStatus { status }),
            ),
            SynchronizerMessage::PullBlocks { ctx, done, heights } => {
                let uuid = Uuid::new_v4();
                self.callback_map.write().insert(uuid, done);
                broadcaster.send(ctx, P2PMessage::from(Message::PullBlocks { uuid, heights }));
            }
            SynchronizerMessage::PullTxsSync { ctx, done, hashes } => {
                let uuid = Uuid::new_v4();
                self.callback_map.write().insert(uuid, done);
                broadcaster.send(ctx, P2PMessage::from(Message::PullTxsSync { uuid, hashes }));
            }
        }

        Box::new(ok(()))
    }
}

impl Synchronizer for OutboundSender {
    fn broadcast_status(&self, status: Status) {
        let msg = OutboundMessage::Synchronizer(SynchronizerMessage::BroadcastStatus { status });

        if let Err(err) = self.try_send(msg) {
            error!("broadcast status failure: {:?}", err);
        }
    }

    fn pull_blocks(
        &self,
        ctx: Context,
        heights: Vec<u64>,
    ) -> FutRuntimeResult<Vec<Block>, SynchronizerError> {
        let (done_tx, done_rx) = channel(1);

        let msg = OutboundMessage::Synchronizer(SynchronizerMessage::PullBlocks {
            ctx,
            done: done_tx,
            heights,
        });

        match self.try_send(msg) {
            Ok(_) => {
                let fut = async move {
                    let mut done_rx = done_rx.compat();

                    if let Some(Ok(box_any)) = await!(done_rx.next()) {
                        if let Ok(box_stxs) = box_any.downcast::<Vec<Block>>() {
                            return Ok(*box_stxs);
                        }
                    }

                    Err(SynchronizerError::Internal("network failure".to_owned()))
                };

                Box::new(fut.boxed().compat())
            }
            Err(e) => Box::new(err(SynchronizerError::Internal(format!(
                "network failure: {:?}",
                e
            )))),
        }
    }

    fn pull_txs_sync(
        &self,
        ctx: Context,
        tx_hashes: &[Hash],
    ) -> FutRuntimeResult<Vec<SignedTransaction>, SynchronizerError> {
        let (done_tx, done_rx) = channel(1);

        let msg = OutboundMessage::Synchronizer(SynchronizerMessage::PullTxsSync {
            ctx,
            done: done_tx,
            hashes: tx_hashes.to_vec(),
        });

        match self.try_send(msg) {
            Ok(_) => {
                let fut = async move {
                    let mut done_rx = done_rx.compat();

                    if let Some(Ok(box_any)) = await!(done_rx.next()) {
                        if let Ok(box_stxs) = box_any.downcast::<Vec<SignedTransaction>>() {
                            return Ok(*box_stxs);
                        }
                    }

                    Err(SynchronizerError::Internal("network failure".to_owned()))
                };

                Box::new(fut.boxed().compat())
            }
            Err(e) => Box::new(err(SynchronizerError::Internal(format!(
                "network failure: {:?}",
                e
            )))),
        }
    }
}
