use super::TxContext;

pub mod message;

use std::{
    error::Error,
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use derive_more::Display;
use futures::{
    channel::mpsc::{
        channel, unbounded, Receiver, Sender, TrySendError, UnboundedReceiver, UnboundedSender,
    },
    lock::Mutex,
    select,
    stream::StreamExt,
};
use futures_timer::Delay;
use log::{debug, error};

use common_crypto::Crypto;
use protocol::{
    fixed_codec::FixedCodec,
    traits::{
        Context, ExecutorFactory, ExecutorParams, Gossip, MemPoolAdapter, PeerTrust, Priority, Rpc,
        ServiceMapping, Storage, TrustFeedback,
    },
    types::{Address, Hash, SignedTransaction, TransactionRequest},
    ProtocolError, ProtocolErrorKind, ProtocolResult,
};

use crate::adapter::message::{
    MsgNewTxs, MsgPullTxs, MsgPushTxs, END_GOSSIP_NEW_TXS, RPC_PULL_TXS,
};
use crate::MemPoolError;

pub const DEFAULT_BROADCAST_TXS_SIZE: usize = 200;
pub const DEFAULT_BROADCAST_TXS_INTERVAL: u64 = 200; // milliseconds

struct IntervalTxsBroadcaster;

impl IntervalTxsBroadcaster {
    pub async fn broadcast<G>(
        stx_rx: UnboundedReceiver<SignedTransaction>,
        interval_reached: Receiver<()>,
        tx_size: usize,
        gossip: G,
        err_tx: UnboundedSender<ProtocolError>,
    ) where
        G: Gossip + Clone + Unpin + 'static,
    {
        let mut stx_rx = stx_rx.fuse();
        let mut interval_rx = interval_reached.fuse();

        let mut txs_cache = Vec::with_capacity(tx_size);

        loop {
            select! {
                opt_stx = stx_rx.next() => {
                    if let Some(stx) = opt_stx {
                        txs_cache.push(stx);

                        if txs_cache.len() == tx_size {
                            Self::do_broadcast(&mut txs_cache, &gossip, err_tx.clone()).await
                        }
                    } else {
                        debug!("mempool: default mempool adapter dropped")
                    }
                },
                signal = interval_rx.next() => {
                    if signal.is_some() {
                        Self::do_broadcast(&mut txs_cache, &gossip, err_tx.clone()).await
                    }
                },
                complete => break,
            };
        }
    }

    pub async fn timer(mut signal_tx: Sender<()>, interval: u64) {
        let interval = Duration::from_millis(interval);

        loop {
            Delay::new(interval).await;

            if let Err(err) = signal_tx.try_send(()) {
                // This means previous interval signal hasn't processed
                // yet, simply drop this one.
                if err.is_full() {
                    debug!("mempool: interval signal channel full");
                }

                if err.is_disconnected() {
                    error!("mempool: interval broadcaster dropped");
                }
            }
        }
    }

    async fn do_broadcast<G>(
        txs_cache: &mut Vec<SignedTransaction>,
        gossip: &G,
        err_tx: UnboundedSender<ProtocolError>,
    ) where
        G: Gossip + Unpin,
    {
        if txs_cache.is_empty() {
            return;
        }

        let batch_stxs = txs_cache.drain(..).collect::<Vec<_>>();
        let gossip_msg = MsgNewTxs { batch_stxs };

        let ctx = Context::new();
        let end = END_GOSSIP_NEW_TXS;

        let report_if_err = move |ret: ProtocolResult<()>| {
            if let Err(err) = ret {
                if err_tx.unbounded_send(err).is_err() {
                    error!("mempool: default mempool adapter dropped");
                }
            }
        };

        report_if_err(
            gossip
                .broadcast(ctx, end, gossip_msg, Priority::Normal)
                .await,
        )
    }
}

pub struct DefaultMemPoolAdapter<EF, C, N, S, DB, Mapping> {
    network:         N,
    storage:         Arc<S>,
    trie_db:         Arc<DB>,
    service_mapping: Arc<Mapping>,

    timeout_gap:  AtomicU64,
    cycles_limit: AtomicU64,
    max_tx_size:  AtomicU64,

    stx_tx: UnboundedSender<SignedTransaction>,
    err_rx: Mutex<UnboundedReceiver<ProtocolError>>,

    pin_c:  PhantomData<C>,
    pin_ef: PhantomData<EF>,
}

impl<EF, C, N, S, DB, Mapping> DefaultMemPoolAdapter<EF, C, N, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping>,
    C: Crypto,
    N: Rpc + PeerTrust + Gossip + Clone + Unpin + 'static,
    S: Storage,
    DB: cita_trie::DB + 'static,
    Mapping: ServiceMapping + 'static,
{
    pub fn new(
        network: N,
        storage: Arc<S>,
        trie_db: Arc<DB>,
        service_mapping: Arc<Mapping>,
        broadcast_txs_size: usize,
        broadcast_txs_interval: u64,
    ) -> Self {
        let (stx_tx, stx_rx) = unbounded();
        let (err_tx, err_rx) = unbounded();
        let (signal_tx, interval_reached) = channel(1);

        tokio::spawn(IntervalTxsBroadcaster::timer(
            signal_tx,
            broadcast_txs_interval,
        ));

        tokio::spawn(IntervalTxsBroadcaster::broadcast(
            stx_rx,
            interval_reached,
            broadcast_txs_size,
            network.clone(),
            err_tx,
        ));

        DefaultMemPoolAdapter {
            network,
            storage,
            trie_db,
            service_mapping,

            timeout_gap: AtomicU64::new(0),
            cycles_limit: AtomicU64::new(0),
            max_tx_size: AtomicU64::new(0),

            stx_tx,
            err_rx: Mutex::new(err_rx),

            pin_c: PhantomData,
            pin_ef: PhantomData,
        }
    }
}

#[async_trait]
impl<EF, C, N, S, DB, Mapping> MemPoolAdapter for DefaultMemPoolAdapter<EF, C, N, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping>,
    C: Crypto + Send + Sync + 'static,
    N: Rpc + PeerTrust + Gossip + Clone + Unpin + 'static,
    S: Storage + 'static,
    DB: cita_trie::DB + 'static,
    Mapping: ServiceMapping + 'static,
{
    #[muta_apm::derive::tracing_span(
        kind = "mempool.adapter",
        logs = "{'txs_len': 'tx_hashes.len()'}"
    )]
    async fn pull_txs(
        &self,
        ctx: Context,
        height: Option<u64>,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let pull_msg = MsgPullTxs {
            height,
            hashes: tx_hashes,
        };

        let resp_msg = self
            .network
            .call::<MsgPullTxs, MsgPushTxs>(ctx, RPC_PULL_TXS, pull_msg, Priority::High)
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
                // Error means receiver channel is empty, is ok here
                Ok(None) | Err(_) => return Ok(()),
            }
        }

        Ok(())
    }

    async fn check_authorization(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()> {
        let network = self.network.clone();
        let network_clone = network.clone();
        let ctx_clone = ctx.clone();
        let tx_clone = tx.clone();

        let blocking_res: Result<ProtocolResult<()>, _> = tokio::task::spawn_blocking(move || {
            // Verify transaction hash
            let fixed_bytes = tx.raw.encode_fixed()?;
            let tx_hash = Hash::digest(fixed_bytes);

            if tx_hash != tx.tx_hash {
                if ctx.is_network_origin_txs() {
                    network.report(
                        ctx,
                        TrustFeedback::Worse(format!(
                            "Mempool wrong tx_hash of tx {:?}",
                            tx.tx_hash
                        )),
                    );
                }

                let wrong_hash = MemPoolError::CheckHash {
                    expect: tx.tx_hash,
                    actual: tx_hash,
                };

                return Err(wrong_hash.into());
            }
            Ok(())
        })
        .await;

        if blocking_res.is_err() || blocking_res.unwrap().is_err() {
            return Err(AdapterError::Internal.into());
        }

        let stx_json = serde_json::to_string(&tx_clone).map_err(|_| {
            network_clone.report(
                ctx_clone.clone(),
                TrustFeedback::Worse(format!("Mempool encode json error {:?}", tx_clone.tx_hash)),
            );
            MemPoolError::EncodeJson
        })?;
        let payload_json =
            serde_json::to_string(&stx_json).map_err(|_| MemPoolError::EncodeJson)?;

        let block = self.storage.get_latest_block(ctx_clone.clone()).await?;
        let caller = Address::from_hex("0x0000000000000000000000000000000000000000")?;
        let executor = EF::from_root(
            block.header.state_root.clone(),
            Arc::clone(&self.trie_db),
            Arc::clone(&self.storage),
            Arc::clone(&self.service_mapping),
        )?;
        let params = ExecutorParams {
            state_root:   block.header.state_root,
            height:       block.header.height,
            timestamp:    block.header.timestamp,
            cycles_limit: 99999,
            proposer:     block.header.proposer,
        };
        let check_resp = executor.read(&params, &caller, 1, &TransactionRequest {
            service_name: "authorization".to_string(),
            method:       "check_authorization".to_string(),
            payload:      payload_json,
        })?;

        if check_resp.is_error() {
            if ctx_clone.is_network_origin_txs() {
                network_clone.report(
                    ctx_clone,
                    TrustFeedback::Worse(format!(
                        "Mempool check authorization failed tx hash {:?}",
                        tx_clone.tx_hash.clone()
                    )),
                )
            }

            return Err(MemPoolError::CheckAuthorization {
                tx_hash:  tx_clone.tx_hash,
                err_info: check_resp.error_message,
            }
            .into());
        }

        Ok(())
    }

    async fn check_transaction(&self, ctx: Context, stx: SignedTransaction) -> ProtocolResult<()> {
        let fixed_bytes = stx.raw.encode_fixed()?;
        let size = fixed_bytes.len() as u64;
        let tx_hash = stx.tx_hash.clone();

        // check tx size
        let max_tx_size = self.max_tx_size.load(Ordering::SeqCst);
        if size > max_tx_size {
            if ctx.is_network_origin_txs() {
                self.network.report(
                    ctx.clone(),
                    TrustFeedback::Bad(format!(
                        "Mempool exceed size limit of tx {:?}",
                        stx.tx_hash
                    )),
                );
            }
            return Err(MemPoolError::ExceedSizeLimit {
                tx_hash,
                max_tx_size,
                size,
            }
            .into());
        }

        // check cycle limit
        let cycles_limit_config = self.cycles_limit.load(Ordering::SeqCst);
        let cycles_limit_tx = stx.raw.cycles_limit;
        if cycles_limit_tx > cycles_limit_config {
            if ctx.is_network_origin_txs() {
                self.network.report(
                    ctx.clone(),
                    TrustFeedback::Bad(format!(
                        "Mempool exceed cycle limit of tx {:?}",
                        stx.tx_hash
                    )),
                );
            }
            return Err(MemPoolError::ExceedCyclesLimit {
                tx_hash,
                cycles_limit_tx,
                cycles_limit_config,
            }
            .into());
        }

        // Verify chain id
        let latest_block = self.storage.get_latest_block(ctx.clone()).await?;
        if latest_block.header.chain_id != stx.raw.chain_id {
            if ctx.is_network_origin_txs() {
                self.network.report(
                    ctx.clone(),
                    TrustFeedback::Worse(format!("Mempool wrong chain of tx {:?}", stx.tx_hash)),
                );
            }
            let wrong_chain_id = MemPoolError::WrongChain {
                tx_hash: stx.tx_hash,
            };

            return Err(wrong_chain_id.into());
        }

        // Verify timeout
        let latest_height = latest_block.header.height;
        let timeout_gap = self.timeout_gap.load(Ordering::SeqCst);

        if stx.raw.timeout > latest_height + timeout_gap {
            if ctx.is_network_origin_txs() {
                self.network.report(
                    ctx.clone(),
                    TrustFeedback::Bad(format!("Mempool invalid timeout of tx {:?}", stx.tx_hash)),
                );
            }
            let invalid_timeout = MemPoolError::InvalidTimeout {
                tx_hash: stx.tx_hash,
            };

            return Err(invalid_timeout.into());
        }

        if stx.raw.timeout < latest_height {
            if ctx.is_network_origin_txs() {
                self.network.report(
                    ctx,
                    TrustFeedback::Bad(format!("Mempool timeout of tx {:?}", stx.tx_hash)),
                );
            }
            let timeout = MemPoolError::Timeout {
                tx_hash: stx.tx_hash,
                timeout: stx.raw.timeout,
            };

            return Err(timeout.into());
        }

        Ok(())
    }

    async fn check_storage_exist(&self, ctx: Context, tx_hash: Hash) -> ProtocolResult<()> {
        match self
            .storage
            .get_transaction_by_hash(ctx, tx_hash.clone())
            .await
        {
            Ok(Some(_)) => Err(MemPoolError::CommittedTx { tx_hash }.into()),
            Ok(None) => Ok(()),
            Err(err) => Err(err),
        }
    }

    async fn get_latest_height(&self, ctx: Context) -> ProtocolResult<u64> {
        let height = self.storage.get_latest_block(ctx).await?.header.height;
        Ok(height)
    }

    async fn get_transactions_from_storage(
        &self,
        ctx: Context,
        block_height: Option<u64>,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<Option<SignedTransaction>>> {
        if let Some(height) = block_height {
            self.storage.get_transactions(ctx, height, tx_hashes).await
        } else {
            let futs = tx_hashes
                .into_iter()
                .map(|tx_hash| self.storage.get_transaction_by_hash(ctx.clone(), tx_hash))
                .collect::<Vec<_>>();
            futures::future::try_join_all(futs).await
        }
    }

    fn report_good(&self, ctx: Context) {
        if ctx.is_network_origin_txs() {
            self.network.report(ctx, TrustFeedback::Good);
        }
    }

    fn set_args(&self, timeout_gap: u64, cycles_limit: u64, max_tx_size: u64) {
        self.timeout_gap.store(timeout_gap, Ordering::Relaxed);
        self.cycles_limit.store(cycles_limit, Ordering::Relaxed);
        self.max_tx_size.store(max_tx_size, Ordering::Relaxed);
    }
}

#[derive(Debug, Display)]
pub enum AdapterError {
    #[display(fmt = "adapter: interval broadcaster drop")]
    IntervalBroadcasterDrop,

    #[display(fmt = "adapter: internal error")]
    Internal,
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

#[cfg(test)]
mod tests {
    use super::IntervalTxsBroadcaster;

    use crate::{adapter::message::MsgNewTxs, tests::default_mock_txs};

    use protocol::{
        traits::{Context, Gossip, MessageCodec, Priority},
        types::Address,
        Bytes, ProtocolResult,
    };

    use async_trait::async_trait;
    use futures::{
        channel::mpsc::{channel, unbounded, UnboundedSender},
        stream::StreamExt,
    };
    use parking_lot::Mutex;

    use std::{
        ops::Sub,
        sync::Arc,
        time::{Duration, Instant},
    };

    #[derive(Clone)]
    struct MockGossip {
        msgs:      Arc<Mutex<Vec<Bytes>>>,
        signal_tx: UnboundedSender<()>,
    }

    impl MockGossip {
        pub fn new(signal_tx: UnboundedSender<()>) -> Self {
            MockGossip {
                msgs: Default::default(),
                signal_tx,
            }
        }
    }

    #[async_trait]
    impl Gossip for MockGossip {
        async fn broadcast<M>(
            &self,
            _: Context,
            _: &str,
            mut msg: M,
            _: Priority,
        ) -> ProtocolResult<()>
        where
            M: MessageCodec,
        {
            let bytes = msg.encode().await.expect("encode message fail");
            self.msgs.lock().push(bytes);

            self.signal_tx
                .unbounded_send(())
                .expect("send broadcast signal fail");

            Ok(())
        }

        async fn users_cast<M>(
            &self,
            _: Context,
            _: &str,
            _: Vec<Address>,
            _: M,
            _: Priority,
        ) -> ProtocolResult<()>
        where
            M: MessageCodec,
        {
            unreachable!()
        }
    }

    macro_rules! pop_msg {
        ($msgs:expr) => {{
            let msg = $msgs.pop().expect("should have one message");
            MsgNewTxs::decode(msg).await.expect("decode MsgNewTxs fail")
        }};
    }

    #[tokio::test]
    async fn test_interval_timer() {
        let (tx, mut rx) = channel(1);
        let interval = Duration::from_millis(200);
        let now = Instant::now();

        tokio::spawn(IntervalTxsBroadcaster::timer(tx, 200));
        rx.next().await.expect("await interval signal fail");

        assert!(now.elapsed().sub(interval).as_millis() < 100u128);
    }

    #[tokio::test]
    async fn test_interval_broadcast_reach_cache_size() {
        let (stx_tx, stx_rx) = unbounded();
        let (err_tx, _err_rx) = unbounded();
        let (_signal_tx, interval_reached) = channel(1);
        let tx_size = 10;
        let (broadcast_signal_tx, mut broadcast_signal_rx) = unbounded();
        let gossip = MockGossip::new(broadcast_signal_tx);

        tokio::spawn(IntervalTxsBroadcaster::broadcast(
            stx_rx,
            interval_reached,
            tx_size,
            gossip.clone(),
            err_tx,
        ));

        for stx in default_mock_txs(11).into_iter() {
            stx_tx.unbounded_send(stx).expect("send stx fail");
        }

        broadcast_signal_rx.next().await;
        let mut msgs = gossip.msgs.lock().drain(..).collect::<Vec<_>>();
        assert_eq!(msgs.len(), 1, "should only have one message");

        let msg = pop_msg!(msgs);
        assert_eq!(msg.batch_stxs.len(), 10, "should only have 10 stx");
    }

    #[tokio::test]
    async fn test_interval_broadcast_reach_interval() {
        let (stx_tx, stx_rx) = unbounded();
        let (err_tx, _err_rx) = unbounded();
        let (signal_tx, interval_reached) = channel(1);
        let tx_size = 10;
        let (broadcast_signal_tx, mut broadcast_signal_rx) = unbounded();
        let gossip = MockGossip::new(broadcast_signal_tx);

        tokio::spawn(IntervalTxsBroadcaster::timer(signal_tx, 200));
        tokio::spawn(IntervalTxsBroadcaster::broadcast(
            stx_rx,
            interval_reached,
            tx_size,
            gossip.clone(),
            err_tx,
        ));

        for stx in default_mock_txs(9).into_iter() {
            stx_tx.unbounded_send(stx).expect("send stx fail");
        }

        broadcast_signal_rx.next().await;
        let mut msgs = gossip.msgs.lock().drain(..).collect::<Vec<_>>();
        assert_eq!(msgs.len(), 1, "should only have one message");

        let msg = pop_msg!(msgs);
        assert_eq!(msg.batch_stxs.len(), 9, "should only have 9 stx");
    }

    #[tokio::test]
    async fn test_interval_broadcast() {
        let (stx_tx, stx_rx) = unbounded();
        let (err_tx, _err_rx) = unbounded();
        let (signal_tx, interval_reached) = channel(1);
        let tx_size = 10;
        let (broadcast_signal_tx, mut broadcast_signal_rx) = unbounded();
        let gossip = MockGossip::new(broadcast_signal_tx);

        tokio::spawn(IntervalTxsBroadcaster::timer(signal_tx, 200));
        tokio::spawn(IntervalTxsBroadcaster::broadcast(
            stx_rx,
            interval_reached,
            tx_size,
            gossip.clone(),
            err_tx,
        ));

        for stx in default_mock_txs(19).into_iter() {
            stx_tx.unbounded_send(stx).expect("send stx fail");
        }

        // Should got two broadcast
        broadcast_signal_rx.next().await;
        broadcast_signal_rx.next().await;

        let mut msgs = gossip.msgs.lock().drain(..).collect::<Vec<_>>();
        assert_eq!(msgs.len(), 2, "should only have two messages");

        let msg = pop_msg!(msgs);
        assert_eq!(
            msg.batch_stxs.len(),
            9,
            "last message should only have 9 stx"
        );

        let msg = pop_msg!(msgs);
        assert_eq!(
            msg.batch_stxs.len(),
            10,
            "first message should only have 10 stx"
        );
    }
}
