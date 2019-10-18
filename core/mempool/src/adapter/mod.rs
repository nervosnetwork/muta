pub mod message;

use std::{
    error::Error,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
    task::{Context as TaskContext, Poll},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::Bytes;
use derive_more::Display;
use futures::{
    channel::mpsc::{unbounded, TrySendError, UnboundedReceiver, UnboundedSender},
    lock::Mutex,
    pin_mut, ready,
    stream::Stream,
    task::AtomicWaker,
};
use futures_timer::Delay;
use log::error;

use common_crypto::Crypto;
use protocol::{
    fixed_codec::ProtocolFixedCodec,
    traits::{Context, Gossip, MemPoolAdapter, Priority, Rpc, Storage},
    types::{Hash, SignedTransaction},
    ProtocolError, ProtocolErrorKind, ProtocolResult,
};

use crate::adapter::message::{
    MsgNewTxs, MsgPullTxs, MsgPushTxs, END_GOSSIP_NEW_TXS, END_RPC_PULL_TXS,
};
use crate::MemPoolError;

pub const DEFAULT_BROADCAST_TXS_SIZE: usize = 200;
pub const DEFAULT_BROADCAST_TXS_INTERVAL: u64 = 200; // milliseconds

struct IntervalTxsBroadcaster<G> {
    waker: AtomicWaker,

    delay:    Delay,
    interval: Duration,

    txs_cache:  Vec<SignedTransaction>,
    cache_size: usize,

    stx_rx: UnboundedReceiver<SignedTransaction>,
    gossip: G,

    err_tx: UnboundedSender<ProtocolError>,
}

impl<G: Gossip + Unpin + Clone + 'static> IntervalTxsBroadcaster<G> {
    fn new(
        interval: Duration,
        cache_size: usize,
        stx_rx: UnboundedReceiver<SignedTransaction>,
        gossip: G,
        err_tx: UnboundedSender<ProtocolError>,
    ) -> Self {
        IntervalTxsBroadcaster {
            waker: AtomicWaker::new(),

            delay: Delay::new(interval),
            interval,

            txs_cache: Vec::with_capacity(cache_size),
            cache_size,

            stx_rx,
            gossip,

            err_tx,
        }
    }

    fn do_broadcast(&mut self) {
        let batch_stxs = self.txs_cache.drain(..).collect::<Vec<_>>();
        let gossip_msg = MsgNewTxs { batch_stxs };

        let gossip = self.gossip.clone();
        let err_tx = self.err_tx.clone();

        let report_if_err = move |ret: ProtocolResult<()>| {
            if let Err(err) = ret {
                if err_tx.unbounded_send(err).is_err() {
                    error!("mempool: default mempool adapter dropped");
                }
            }
        };

        runtime::spawn(async move {
            let ctx = Context::new();
            let end = END_GOSSIP_NEW_TXS;

            report_if_err(
                gossip
                    .broadcast(ctx, end, gossip_msg, Priority::Normal)
                    .await,
            );
        });
    }
}

impl<G: Gossip + Unpin + Clone + 'static> Future for IntervalTxsBroadcaster<G> {
    type Output = <Delay as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        self.waker.register(ctx.waker());

        macro_rules! loop_break {
            ($poll:expr) => {
                match $poll {
                    Poll::Pending => break,
                    Poll::Ready(Some(v)) => v,
                    Poll::Ready(None) => return Poll::Ready(()),
                }
            };
        }

        // Insert received SignedTransaction first, if we reach specifed size,
        // broadcast them.
        loop {
            let stx_rx = &mut self.as_mut().stx_rx;
            pin_mut!(stx_rx);

            let stx = loop_break!(stx_rx.poll_next(ctx));
            self.txs_cache.push(stx);

            if self.txs_cache.len() == self.cache_size {
                self.do_broadcast();
            }
        }

        // Check if we reach next broadcast interval
        loop {
            let delay = &mut self.as_mut().delay;
            pin_mut!(delay);

            ready!(delay.poll(ctx));

            if !self.txs_cache.is_empty() {
                self.do_broadcast();
            }

            let interval = self.interval;

            self.delay.reset(Instant::now() + interval);
            self.waker.wake();
        }
    }
}

pub struct DefaultMemPoolAdapter<C, N, S> {
    network: N,
    storage: Arc<S>,

    timeout_gap: AtomicU64,

    stx_tx: UnboundedSender<SignedTransaction>,
    err_rx: Mutex<UnboundedReceiver<ProtocolError>>,

    pin_c: PhantomData<C>,
}

impl<C, N, S> DefaultMemPoolAdapter<C, N, S>
where
    C: Crypto,
    N: Rpc + Gossip + Clone + Unpin + 'static,
    S: Storage,
{
    pub fn new(
        network: N,
        storage: Arc<S>,
        timeout_gap: u64,
        broadcast_txs_size: usize,
        broadcast_txs_interval: u64,
    ) -> Self {
        let (stx_tx, stx_rx) = unbounded();
        let (err_tx, err_rx) = unbounded();

        let interval = Duration::from_millis(broadcast_txs_interval);
        let cache_size = broadcast_txs_size;

        let broadcaster =
            IntervalTxsBroadcaster::new(interval, cache_size, stx_rx, network.clone(), err_tx);

        runtime::spawn(broadcaster);

        DefaultMemPoolAdapter {
            network,
            storage,

            timeout_gap: AtomicU64::new(timeout_gap),

            stx_tx,
            err_rx: Mutex::new(err_rx),

            pin_c: PhantomData,
        }
    }
}

#[async_trait]
impl<C, N, S> MemPoolAdapter for DefaultMemPoolAdapter<C, N, S>
where
    C: Crypto + Send + Sync + 'static,
    N: Rpc + Gossip + Clone + Unpin + 'static,
    S: Storage + 'static,
{
    async fn pull_txs(
        &self,
        ctx: Context,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let pull_msg = MsgPullTxs { hashes: tx_hashes };

        let resp_msg = self
            .network
            .call::<MsgPullTxs, MsgPushTxs>(ctx, END_RPC_PULL_TXS, pull_msg, Priority::High)
            .await?;

        Ok(resp_msg.sig_txs)
    }

    async fn broadcast_tx(&self, _ctx: Context, stx: SignedTransaction) -> ProtocolResult<()> {
        self.stx_tx
            .unbounded_send(stx)
            .map_err(AdapterError::from)?;

        if let Some(mut err_rx) = self.err_rx.try_lock() {
            match err_rx.try_next() {
                Ok(Some(err)) => return Err(err),
                Ok(None) => return Ok(()),
                Err(_) => return Err(ProtocolError::from(AdapterError::IntervalBroadcasterDrop)),
            }
        }

        Ok(())
    }

    async fn check_signature(&self, _ctx: Context, tx: SignedTransaction) -> ProtocolResult<()> {
        let hash = tx.tx_hash.as_bytes();
        let pub_key = tx.pubkey.as_ref();
        let sig = tx.signature.as_ref();

        C::verify_signature(hash.as_ref(), sig, pub_key).map_err(|_| {
            MemPoolError::CheckSig {
                tx_hash: tx.tx_hash,
            }
            .into()
        })
    }

    // TODO: Verify Fee?
    // TODO: Verify Nonce?
    // TODO: Cycle limit?
    async fn check_transaction(&self, _ctx: Context, stx: SignedTransaction) -> ProtocolResult<()> {
        // Verify transaction hash
        let fixed_bytes = stx.raw.encode_fixed()?;
        let tx_hash = Hash::digest(fixed_bytes);

        if tx_hash != stx.tx_hash {
            let wrong_hash = MemPoolError::CheckHash {
                expect: stx.tx_hash,
                actual: tx_hash,
            };

            return Err(wrong_hash.into());
        }

        // Verify chain id
        let latest_epoch = self.storage.get_latest_epoch().await?;
        if latest_epoch.header.chain_id != stx.raw.chain_id {
            let wrong_chain_id = MemPoolError::WrongChain {
                tx_hash: stx.tx_hash,
            };

            return Err(wrong_chain_id.into());
        }

        // Verify timeout
        let latest_epoch_id = latest_epoch.header.epoch_id;
        let timeout_gap = self.timeout_gap.load(Ordering::SeqCst);

        if stx.raw.timeout > latest_epoch_id + timeout_gap {
            let invalid_timeout = MemPoolError::InvalidTimeout {
                tx_hash: stx.tx_hash,
            };

            return Err(invalid_timeout.into());
        }

        if stx.raw.timeout < latest_epoch_id {
            let timeout = MemPoolError::Timeout {
                tx_hash: stx.tx_hash,
                timeout: stx.raw.timeout,
            };

            return Err(timeout.into());
        }

        Ok(())
    }

    async fn check_storage_exist(&self, _ctx: Context, tx_hash: Hash) -> ProtocolResult<()> {
        match self.storage.get_transaction_by_hash(tx_hash.clone()).await {
            Ok(_) => Err(MemPoolError::CommittedTx { tx_hash }.into()),
            Err(err) => {
                // TODO: downcast to StorageError
                if err.to_string().contains("GetNone") {
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn get_latest_epoch_id(&self, _ctx: Context) -> ProtocolResult<u64> {
        let epoch_id = self.storage.get_latest_epoch().await?.header.epoch_id;
        Ok(epoch_id)
    }
}

#[derive(Debug, Display)]
pub enum AdapterError {
    #[display(fmt = "adapter: interval broadcaster drop")]
    IntervalBroadcasterDrop,
}

impl Error for AdapterError {}

impl<T> From<TrySendError<T>> for AdapterError {
    fn from(_error: TrySendError<T>) -> AdapterError {
        AdapterError::IntervalBroadcasterDrop
    }
}

impl From<AdapterError> for ProtocolError {
    fn from(error: AdapterError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Mempool, Box::new(error))
    }
}
