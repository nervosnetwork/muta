use std::clone::Clone;
use std::sync::Arc;

use futures::compat::Future01CompatExt;
use futures::prelude::{FutureExt, StreamExt, TryFutureExt};
use futures::stream;
use log::error;

use common_channel::Sender;
use core_context::Context;
use core_network_message::{
    common::{PullTxs, PushTxs},
    tx_pool::BroadcastTxs,
};
use core_network_message::{Codec, Method};
use core_runtime::TransactionPool;
use core_types::SignedTransaction;

use crate::common::scope_from_context;
use crate::{BytesBroadcaster, CallbackMap, Error, OutboundHandle};

pub struct TxPoolReactor<P> {
    outbound: OutboundHandle,
    callback: Arc<CallbackMap>,

    tx_pool: Arc<P>,
}

impl<P> Clone for TxPoolReactor<P> {
    fn clone(&self) -> Self {
        TxPoolReactor {
            outbound: self.outbound.clone(),
            callback: Arc::clone(&self.callback),

            tx_pool: Arc::clone(&self.tx_pool),
        }
    }
}

impl<P> TxPoolReactor<P>
where
    P: TransactionPool + 'static,
{
    pub fn new(outbound: OutboundHandle, callback: Arc<CallbackMap>, tx_pool: Arc<P>) -> Self {
        TxPoolReactor {
            outbound,
            callback,

            tx_pool,
        }
    }

    pub async fn react(&self, ctx: Context, method: Method, data: Vec<u8>) -> Result<(), Error> {
        match method {
            Method::BroadcastTxs => self.handle_broadcast_txs(ctx, data).await?,
            Method::PullTxs => self.handle_pull_txs(ctx, data).await?,
            Method::PushTxs => self.handle_push_txs(ctx, data).await?,
            _ => Err(Error::UnknownMethod(method.to_u32()))?,
        };

        Ok(())
    }

    pub async fn handle_broadcast_txs(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let broadcast_txs = <BroadcastTxs as Codec>::decode(data.as_slice())?;
        let mut sig_txs = stream::iter(broadcast_txs.des()?);

        while let Some(stx) = sig_txs.next().await {
            let ctx = ctx.clone();
            let tx_pool = Arc::clone(&self.tx_pool);

            let insert = async move {
                if let Err(err) = tx_pool.insert(ctx, stx.untx).compat().await {
                    error!(
                        "net [inbound]: tx_pool: [hash: {:?}, err: {:?}]",
                        stx.hash, err
                    );
                }
            };

            tokio::spawn(insert.unit_error().boxed().compat());
        }

        Ok(())
    }

    pub async fn handle_pull_txs(&self, ctx: Context, data: Vec<u8>) -> Result<(), Error> {
        let pull_txs = <PullTxs as Codec>::decode(data.as_slice())?;
        let uid = pull_txs.uid;
        let hashes = pull_txs.des()?;

        let txs = self
            .tx_pool
            .get_batch(ctx.clone(), hashes.as_slice())
            .compat()
            .await?;
        let push_txs = PushTxs::from(uid, txs);

        let scope = scope_from_context(ctx).ok_or(Error::SessionIdNotFound)?;
        if let Err(err) = self
            .outbound
            .quick_filter_broadcast(Method::PushTxs, push_txs, scope)
        {
            log::warn!("net [inbound]: pull_txs: [err: {:?}]", err);
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
