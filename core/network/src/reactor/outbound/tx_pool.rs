use std::any::Any;
use std::marker::Send;
use std::sync::Arc;

use futures::future::ok;
use futures::sync::mpsc::Sender;
use uuid::Uuid;

use core_context::Context;
use core_types::{Hash, SignedTransaction};

use crate::p2p::{Broadcaster, Message as P2PMessage};
use crate::reactor::{CallbackMap, FutReactorResult, Reactor};
use crate::Message;

#[derive(Clone, Debug)]
pub enum TransactionPoolMessage {
    BroadcastTxs {
        txs: Vec<SignedTransaction>,
    },
    PullTxs {
        ctx:    Context,
        done:   Sender<Box<dyn Any + Send + 'static>>,
        hashes: Vec<Hash>,
    },
}

pub struct TransactionPoolReactor {
    callback_map: CallbackMap,
}

impl TransactionPoolReactor {
    pub fn new(callback_map: CallbackMap) -> Self {
        TransactionPoolReactor { callback_map }
    }

    pub fn do_broadcast(broadcaster: &mut Broadcaster, txs: Vec<SignedTransaction>) {
        broadcaster.send(
            Context::new(),
            P2PMessage::from(Message::BroadcastTxs { txs }),
        )
    }

    pub fn do_pull_txs(
        broadcaster: &mut Broadcaster,
        callback_map: CallbackMap,
        ctx: Context,
        done: Sender<Box<dyn Any + Send + 'static>>,
        hashes: Vec<Hash>,
    ) {
        let uuid = Uuid::new_v4();
        callback_map.write().insert(uuid, done);
        broadcaster.send(ctx, P2PMessage::from(Message::PullTxs { uuid, hashes }));
    }
}

impl Reactor for TransactionPoolReactor {
    type Input = TransactionPoolMessage;
    type Output = FutReactorResult;

    fn react(&mut self, mut broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        match input {
            TransactionPoolMessage::BroadcastTxs { txs } => {
                Self::do_broadcast(&mut broadcaster, txs);
            }
            TransactionPoolMessage::PullTxs { ctx, done, hashes } => {
                let callback_map = Arc::clone(&self.callback_map);
                Self::do_pull_txs(&mut broadcaster, callback_map, ctx, done, hashes);
            }
        }

        Box::new(ok(()))
    }
}

pub mod impl_comp {
    use std::fmt::Debug;

    use futures::future::err;
    use futures::sync::mpsc::channel;
    use futures03::{
        compat::Stream01CompatExt,
        prelude::{FutureExt, StreamExt, TryFutureExt},
    };
    use log::error;

    use core_context::Context;
    use core_runtime::{FutRuntimeResult, TransactionPoolError};
    use core_types::{Hash, SignedTransaction};

    use components_transaction_pool::Broadcaster;

    use crate::reactor::outbound::{OutboundMessage, Sender};

    use super::TransactionPoolMessage;

    impl Broadcaster for Sender {
        fn broadcast_batch(&mut self, txs: Vec<SignedTransaction>) {
            let msg =
                OutboundMessage::TransactionPool(TransactionPoolMessage::BroadcastTxs { txs });

            if let Err(err) = self.try_send(msg) {
                handle_error(err);
            }
        }

        fn pull_txs(
            &mut self,
            ctx: Context,
            hashes: Vec<Hash>,
        ) -> FutRuntimeResult<Vec<SignedTransaction>, TransactionPoolError> {
            // TODO: timeout
            let (done_tx, done_rx) = channel(1);

            let msg = OutboundMessage::TransactionPool(TransactionPoolMessage::PullTxs {
                ctx,
                done: done_tx,
                hashes,
            });

            if self.try_send(msg).map_err(handle_error).is_ok() {
                let fut = async move {
                    let mut done_rx = done_rx.compat();

                    if let Some(Ok(box_any)) = await!(done_rx.next()) {
                        if let Ok(box_stxs) = box_any.downcast::<Vec<SignedTransaction>>() {
                            return Ok(*box_stxs);
                        }
                    }

                    Err(TransactionPoolError::Internal("network failure".to_owned()))
                };

                Box::new(fut.boxed().compat())
            } else {
                Box::new(err(TransactionPoolError::Internal(
                    "network failure".to_owned(),
                )))
            }
        }
    }

    fn handle_error<E: Debug>(err: E) {
        error!("transaction pool: broadcast failure: {:?}", err);
    }
}
