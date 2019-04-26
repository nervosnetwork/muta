use std::convert::TryInto;
use std::sync::Arc;

use core_context::Context;
use core_runtime::{TransactionPool, TransactionPoolError};
use core_serialization as ser;
use core_types::{Hash, SignedTransaction, TypesError};
use futures03::compat::Future01CompatExt;
use futures03::prelude::{FutureExt, TryFutureExt};
use log::error;
use uuid::Uuid;

use crate::p2p::message::tx_pool::{packed_message, BroadcastTxs, PullTxs, PushTxs, TxPoolMessage};
use crate::p2p::{self as p2p, Broadcaster};
use crate::reactor::{CallbackMap, Reaction, Reactor, ReactorMessage};

#[derive(Debug)]
enum Error {
    Serialization(ser::CodecError),
    CoreType(TypesError),
    TransactionPool(TransactionPoolError),
}

pub struct TransactionPoolReactor<P> {
    tx_pool:      Arc<P>,
    callback_map: CallbackMap,
}

impl<P> Reactor for TransactionPoolReactor<P>
where
    P: TransactionPool + 'static,
{
    type Input = (Context, TxPoolMessage);
    type Output = Reaction<ReactorMessage>;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        if let (ctx, TxPoolMessage { message: Some(msg) }) = input {
            match msg {
                packed_message::Message::BroadcastTxs(BroadcastTxs { txs }) => {
                    let fut = Self::insert_txs(ctx.clone(), Arc::clone(&self.tx_pool), txs);

                    Reaction::Done(Box::new(fut.unit_error().boxed().compat()))
                }
                packed_message::Message::PullTxs(pull_msg) => {
                    let fut = Self::pull_txs(
                        ctx.clone(),
                        Arc::clone(&self.tx_pool),
                        broadcaster.clone(),
                        pull_msg,
                    );

                    Reaction::Done(Box::new(fut.unit_error().boxed().compat()))
                }
                packed_message::Message::PushTxs(push_msg) => {
                    let fut = Self::push_txs(ctx.clone(), Arc::clone(&self.callback_map), push_msg);

                    Reaction::Done(Box::new(fut.unit_error().boxed().compat()))
                }
            }
        } else {
            unreachable!()
        }
    }
}

impl<P> TransactionPoolReactor<P>
where
    P: TransactionPool + 'static,
{
    pub fn new(tx_pool: Arc<P>, callback_map: CallbackMap) -> Self {
        TransactionPoolReactor {
            tx_pool,
            callback_map,
        }
    }

    async fn insert_ser_stx(
        ctx: Context,
        tx_pool: Arc<P>,
        ser_stx: ser::SignedTransaction,
    ) -> Result<(), Error> {
        let stx = TryInto::<SignedTransaction>::try_into(ser_stx).map_err(Error::Serialization)?;
        let hash = stx.hash;
        let untx = stx.untx;

        await!(tx_pool.insert(ctx, hash, untx).compat())
            .map_err(Error::TransactionPool)
            .map(|_| ())
    }

    async fn insert_txs(ctx: Context, tx_pool: Arc<P>, ser_txs: Vec<ser::SignedTransaction>) {
        for ser_stx in ser_txs.into_iter() {
            let _ = await!(Self::insert_ser_stx(
                ctx.clone(),
                Arc::clone(&tx_pool),
                ser_stx
            ))
            .map_err(|err| {
                error!("network: tx_pool reactor: insert tx failure: {:?}", err);
            });
        }
    }

    async fn get_batch_txs(
        ctx: Context,
        tx_pool: Arc<P>,
        hashes_bytes: Vec<Vec<u8>>,
    ) -> Result<Vec<ser::SignedTransaction>, Error> {
        let hashes = hashes_bytes
            .into_iter()
            .map(|h| Hash::from_bytes(h.as_slice()).map_err(Error::CoreType))
            .collect::<Result<Vec<Hash>, Error>>()?;

        let maybe_sig_txs = await!(tx_pool.get_batch(ctx, hashes.as_slice()).compat())
            .map_err(Error::TransactionPool);

        maybe_sig_txs.map(|sig_txs| {
            sig_txs
                .into_iter()
                .map(From::from)
                .collect::<Vec<ser::SignedTransaction>>()
        })
    }

    async fn pull_txs(
        ctx: Context,
        tx_pool: Arc<P>,
        mut broadcaster: Broadcaster,
        pull_msg: PullTxs,
    ) {
        let PullTxs { uuid, hashes } = pull_msg;

        let sig_txs = await!(Self::get_batch_txs(ctx.clone(), tx_pool, hashes))
            .map_err(|err| {
                error!("network: tx_pool reactor: get batch txs failure: {:?}", err);
            })
            .unwrap_or_default();

        broadcaster.send(
            ctx,
            p2p::Message::TxPoolMessage(TxPoolMessage::push_txs(uuid, sig_txs)),
        )
    }

    // FIXME: handle error
    async fn push_txs(_: Context, callback_map: CallbackMap, push_msg: PushTxs) {
        let PushTxs { uuid, sig_txs } = push_msg;

        if let Ok(uuid) = Uuid::parse_str(uuid.as_str()) {
            if let Some(mut done_tx) = callback_map.write().remove(&uuid) {
                let maybe_sig_txs = sig_txs
                    .into_iter()
                    .map(|ser_stx| {
                        TryInto::<SignedTransaction>::try_into(ser_stx)
                            .map_err(Error::Serialization)
                    })
                    .collect::<Result<Vec<SignedTransaction>, Error>>();

                if let Ok(sig_txs) = maybe_sig_txs {
                    if let Err(err) = done_tx.try_send(Box::new(sig_txs)) {
                        error!("network: push_tx msg: send data back failure: {:?}", err);
                    }
                }
            }
        } else {
            error!("network: push_tx msg: bad uuid: {:?}", uuid);
        }
    }
}
