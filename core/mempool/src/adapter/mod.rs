pub mod message;

use std::{
    marker::PhantomData,
    mem,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    sync::Arc,
    time::{Instant, Duration},
    task::{Context as TaskContext, Poll},
    future::Future,
    pin::Pin,
};

use futures::{task::AtomicWaker, ready, pin_mut, channel::mpsc::{UnboundedReceiver, unbounded}};
use futures_timer::Delay;
use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::Mutex;

use common_crypto::Crypto;
use protocol::{
    fixed_codec::ProtocolFixedCodec,
    traits::{Context, Gossip, MemPoolAdapter, Priority, Rpc, Storage},
    types::{Hash, SignedTransaction},
    ProtocolResult,
};

use crate::adapter::message::{
    MsgNewTxs, MsgPullTxs, MsgPushTxs, END_GOSSIP_NEW_TXS, END_RPC_PULL_TXS,
};
use crate::MemPoolError;

pub const DEFAULT_BROADCAST_TXS_SIZE: usize = 200;
pub const DEFAULT_BROADCAST_TXS_INTERVAL: u64 = 200; // milliseconds

struct IntervalTxsBroadcaster<G> {
    waker: AtomicWaker,

    delay:  Delay,
    interval: Duration,

    txs_cache: Vec<SignedTransaction>,
    cache_size: usize,

    gossip: G,
}

impl<G: Gossip + Unpin> IntervalTxsBroadcaster<G> {
    pub fn new(gossip: G, interval: Duration, cache_size: usize, txs_cache: Arc<Mutex<Vec<SignedTransaction>>>) -> Self {
        IntervalTxsBroadcaster {
            waker: AtomicWaker::new(),
            interval,
            delay: Delay::new(interval),

            txs_cache,
            cache_size,

            gossip,
        }
    }
}

impl<G: Gossip + Unpin> Future for IntervalTxsBroadcaster<G> {
    type Output = <Delay as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let interval = self.interval;
        let cache_size = self.cache_size;

        loop {
            let delay = &mut self.as_mut().delay;
            pin_mut!(delay);

            let _ = ready!(delay.poll(ctx));

            {
                let mut txs_cache = self.txs_cache.lock();

                if !txs_cache.is_empty() {
                    let batch_stxs = mem::replace(&mut *txs_cache, Vec::with_capacity(cache_size));
                    let gossip_msg = MsgNewTxs { batch_stxs };

                    self.gossip
                        .broadcast(Context::new(), END_GOSSIP_NEW_TXS, gossip_msg, Priority::Normal))
                }
            }


            self.delay.reset(Instant::now() + interval);
            self.waker.wake();
        }

        Poll::Pending
    }
}

pub struct DefaultMemPoolAdapter<C, N, S> {
    network: N,
    storage: Arc<S>,

    txs_cache:    Mutex<Vec<SignedTransaction>>,
    broadcast_at: Mutex<Instant>,

    timeout_gap:            AtomicU64,
    broadcast_txs_size:     AtomicUsize,
    broadcast_txs_interval: AtomicU64, // second

    pin_c: PhantomData<C>,
}

impl<C, N, S> DefaultMemPoolAdapter<C, N, S>
where
    C: Crypto,
    N: Rpc + Gossip,
    S: Storage,
{
    pub fn new(
        network: N,
        storage: Arc<S>,
        timeout_gap: u64,
        broadcast_txs_size: usize,
        broadcast_txs_interval: u64,
    ) -> Self {
        DefaultMemPoolAdapter {
            network,
            storage,

            txs_cache: Mutex::new(Vec::with_capacity(broadcast_txs_size)),
            broadcast_at: Mutex::new(Instant::now()),

            timeout_gap: AtomicU64::new(timeout_gap),
            broadcast_txs_size: AtomicUsize::new(broadcast_txs_size),
            broadcast_txs_interval: AtomicU64::new(broadcast_txs_interval),

            pin_c: PhantomData,
        }
    }
}

#[async_trait]
impl<C, N, S> MemPoolAdapter for DefaultMemPoolAdapter<C, N, S>
where
    C: Crypto + Send + Sync + 'static,
    N: Rpc + Gossip + 'static,
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

    async fn broadcast_tx(&self, ctx: Context, stx: SignedTransaction) -> ProtocolResult<()> {
        // broadcast txs size and interval never change
        let broadcast_txs_size = self.broadcast_txs_size.load(Ordering::Relaxed);
        let broadcast_txs_interval = self.broadcast_txs_interval.load(Ordering::Relaxed);

        let batch_stxs = {
            let mut txs_cache = self.txs_cache.lock();

            txs_cache.push(stx);

            // Refresh broadcast_at
            if txs_cache.len() == 1 {
                *self.broadcast_at.lock() = Instant::now();
            }

            if self.broadcast_at.lock().elapsed().as_secs() >= broadcast_txs_interval
                || txs_cache.len() >= broadcast_txs_size
            {
                mem::replace(&mut *txs_cache, Vec::with_capacity(broadcast_txs_size))
            } else {
                return Ok(());
            }
        };

        let gossip_msg = MsgNewTxs { batch_stxs };

        self.network
            .broadcast(ctx, END_GOSSIP_NEW_TXS, gossip_msg, Priority::Normal)
            .await
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
