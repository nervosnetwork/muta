#![feature(async_closure, test)]
#![allow(clippy::suspicious_else_formatting)]

mod adapter;
mod context;
mod map;
#[cfg(test)]
mod tests;
mod tx_cache;

pub use adapter::message::{
    MsgNewTxs, MsgPushTxs, NewTxsHandler, PullTxsHandler, END_GOSSIP_NEW_TXS, RPC_PULL_TXS,
    RPC_RESP_PULL_TXS, RPC_RESP_PULL_TXS_SYNC,
};
pub use adapter::DefaultMemPoolAdapter;
pub use adapter::{DEFAULT_BROADCAST_TXS_INTERVAL, DEFAULT_BROADCAST_TXS_SIZE};

use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::collections::HashSet;

use async_trait::async_trait;
use derive_more::Display;
use futures::future::try_join_all;
use tokio::sync::RwLock;

use protocol::traits::{Context, MemPool, MemPoolAdapter, MixedTxHashes};
use protocol::types::{Hash, SignedTransaction};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::context::TxContext;
use crate::map::Map;
use crate::tx_cache::TxCache;

/// Memory pool for caching transactions.
pub struct HashMemPool<Adapter: MemPoolAdapter> {
    /// Pool size limit.
    pool_size:      usize,
    /// A system param limits the life time of an off-chain transaction.
    timeout_gap:    AtomicU64,
    /// A structure for caching new transactions and responsible transactions of
    /// propose-sync.
    tx_cache:       TxCache,
    /// A structure for caching fresh transactions in order transaction hashes.
    callback_cache: Arc<Map<SignedTransaction>>,
    /// Supply necessary functions from outer modules.
    adapter:        Arc<Adapter>,
    /// exclusive flush_memory and insert_tx to avoid repeat txs insertion.
    flush_lock:     RwLock<()>,
}

impl<Adapter: 'static> HashMemPool<Adapter>
where
    Adapter: MemPoolAdapter,
{
    pub fn new(pool_size: usize, adapter: Adapter) -> Self {
        HashMemPool {
            pool_size,
            timeout_gap: AtomicU64::new(0),
            tx_cache: TxCache::new(pool_size * 2),
            callback_cache: Arc::new(Map::new(pool_size)),
            adapter: Arc::new(adapter),
            flush_lock: RwLock::new(()),
        }
    }

    pub fn get_tx_cache(&self) -> &TxCache {
        &self.tx_cache
    }

    pub fn get_callback_cache(&self) -> &Map<SignedTransaction> {
        &self.callback_cache
    }

    pub fn get_adapter(&self) -> &Adapter {
        &self.adapter
    }

    async fn show_unknown_txs(&self, tx_hashes: Vec<Hash>) -> Vec<Hash> {
        let tx_hashes = self.tx_cache.show_unknown(tx_hashes).await;
        let mut unknown_hashes = vec![];

        for tx_hash in tx_hashes.into_iter() {
            if !self.callback_cache.contains_key(&tx_hash).await {
                unknown_hashes.push(tx_hash)
            }
        }

        unknown_hashes
    }

    async fn insert_tx(
        &self,
        ctx: Context,
        tx: SignedTransaction,
        tx_type: TxType,
    ) -> ProtocolResult<()> {
        let _lock = self.flush_lock.read().await;

        let tx_hash = &tx.tx_hash;
        self.tx_cache.check_reach_limit(self.pool_size).await?;
        self.tx_cache.check_exist(tx_hash).await?;
        self.adapter
            .check_authorization(ctx.clone(), tx.clone())
            .await?;
        self.adapter
            .check_transaction(ctx.clone(), tx.clone())
            .await?;
        self.adapter
            .check_storage_exist(ctx.clone(), tx_hash.clone())
            .await?;

        match tx_type {
            TxType::NewTx => self.tx_cache.insert_new_tx(tx.clone()).await?,
            TxType::ProposeTx => self.tx_cache.insert_propose_tx(tx.clone()).await?,
        }

        if !ctx.is_network_origin_txs() {
            self.adapter.broadcast_tx(ctx, tx).await?;
        } else {
            self.adapter.report_good(ctx);
        }

        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "mempool", logs = "{'txs': 'txs.len()'}")]
    async fn verify_tx_in_parallel(
        &self,
        ctx: Context,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let now = Instant::now();
        let len = txs.len();

        let futs = txs
            .into_iter()
            .map(|signed_tx| {
                let adapter = Arc::clone(&self.adapter);
                let ctx = ctx.clone();

                tokio::spawn(async move {
                    adapter
                        .check_authorization(ctx.clone(), signed_tx.clone())
                        .await?;
                    adapter
                        .check_transaction(ctx.clone(), signed_tx.clone())
                        .await?;
                    adapter
                        .check_storage_exist(ctx.clone(), signed_tx.tx_hash.clone())
                        .await
                })
            })
            .collect::<Vec<_>>();
        try_join_all(futs).await.map_err(|e| {
            log::error!("[mempool] verify batch txs error {:?}", e);
            MemPoolError::VerifyBatchTransactions
        })?;

        log::info!(
            "[mempool] verify txs done, size {:?} cost {:?}",
            len,
            now.elapsed()
        );
        Ok(())
    }
}

#[async_trait]
impl<Adapter: 'static> MemPool for HashMemPool<Adapter>
where
    Adapter: MemPoolAdapter,
{
    async fn insert(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()> {
        self.insert_tx(ctx, tx, TxType::NewTx).await
    }

    #[muta_apm::derive::tracing_span(
        kind = "mempool",
        logs = "{'cycles_limit': 'cycles_limit', 'tx_num_limit': 'tx_num_limit'}"
    )]
    async fn package(
        &self,
        ctx: Context,
        cycles_limit: u64,
        tx_num_limit: u64,
    ) -> ProtocolResult<MixedTxHashes> {
        let current_height = self.adapter.get_latest_height(ctx.clone()).await?;
        log::info!(
            "[core_mempool]: {:?} txs in map and {:?} txs in queue while package",
            self.tx_cache.len().await,
            self.tx_cache.queue_len(),
        );
        let inst = Instant::now();
        let result = self
            .tx_cache
            .package(
                cycles_limit,
                tx_num_limit,
                current_height,
                current_height + self.timeout_gap.load(Ordering::Relaxed),
            )
            .await;
        match result {
            Ok(txs) => {
                common_apm::metrics::mempool::MEMPOOL_PACKAGE_SIZE_VEC_STATIC
                    .package
                    .observe((txs.order_tx_hashes.len()) as f64);
                common_apm::metrics::mempool::MEMPOOL_TIME_STATIC
                    .package
                    .observe(common_apm::metrics::duration_to_sec(inst.elapsed()));
                Ok(txs)
            }
            Err(e) => {
                common_apm::metrics::mempool::MEMPOOL_RESULT_COUNTER_STATIC
                    .package
                    .failure
                    .inc();
                Err(e)
            }
        }
    }

    #[muta_apm::derive::tracing_span(
        kind = "mempool",
        logs = "{'tx_len':
     'tx_hashes.len()'}"
    )]
    async fn flush(&self, ctx: Context, tx_hashes: Vec<Hash>) -> ProtocolResult<()> {
        let _lock = self.flush_lock.write().await;

        let current_height = self.adapter.get_latest_height(ctx.clone()).await?;
        log::info!(
            "[core_mempool]: flush mempool with {:?} tx_hashes",
            tx_hashes.len(),
        );
        self.tx_cache
            .flush(
                &tx_hashes,
                current_height,
                current_height + self.timeout_gap.load(Ordering::Relaxed),
            )
            .await;
        self.callback_cache.clear().await;

        Ok(())
    }

    #[muta_apm::derive::tracing_span(
        kind = "mempool",
        logs = "{'tx_len':
     'tx_hashes.len()'}"
    )]
    async fn get_full_txs(
        &self,
        ctx: Context,
        height: Option<u64>,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let len = tx_hashes.len();
        let mut missing_hashes = vec![];
        let mut full_txs = Vec::with_capacity(len);

        for tx_hash in tx_hashes.iter() {
            if let Some(tx) = self.tx_cache.get(tx_hash).await {
                full_txs.push(tx);
            } else if let Some(tx) = self.callback_cache.get(tx_hash).await {
                full_txs.push(tx);
            } else {
                missing_hashes.push(tx_hash.clone());
            }
        }

        // for push txs when local mempool is flushed, but the remote node still fetch
        // full block
        if !missing_hashes.is_empty() {
            let txs = self
                .adapter
                .get_transactions_from_storage(ctx, height, missing_hashes)
                .await?;
            let txs = txs
                .into_iter()
                .filter_map(|opt_tx| opt_tx)
                .collect::<Vec<_>>();

            full_txs.extend(txs);
        }

        if full_txs.len() != len {
            Err(MemPoolError::MisMatch {
                require:  len,
                response: full_txs.len(),
            }
            .into())
        } else {
            Ok(full_txs)
        }
    }

    #[muta_apm::derive::tracing_span(
        kind = "mempool",
        logs = "{'tx_len': 'order_tx_hashes.len()'}"
    )]
    async fn ensure_order_txs(
        &self,
        ctx: Context,
        height: Option<u64>,
        order_tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<()> {
        check_dup_order_hashes(&order_tx_hashes)?;

        let unknown_hashes = self.show_unknown_txs(order_tx_hashes).await;
        if !unknown_hashes.is_empty() {
            let unknown_len = unknown_hashes.len();
            let txs = self
                .adapter
                .pull_txs(ctx.clone(), height, unknown_hashes)
                .await?;
            // Make sure response signed_txs is the same size of request hashes.
            if txs.len() != unknown_len {
                return Err(MemPoolError::EnsureBreak {
                    require:  unknown_len,
                    response: txs.len(),
                }
                .into());
            }

            self.verify_tx_in_parallel(ctx.clone(), txs.clone()).await?;
            for signed_tx in txs.into_iter() {
                self.callback_cache
                    .insert(signed_tx.tx_hash.clone(), signed_tx)
                    .await;
            }

            self.adapter.report_good(ctx);
        }

        Ok(())
    }

    #[muta_apm::derive::tracing_span(
        kind = "mempool",
        logs = "{'tx_len': 'propose_tx_hashes.len()'}"
    )]
    async fn sync_propose_txs(
        &self,
        ctx: Context,
        propose_tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<()> {
        let unknown_hashes = self.show_unknown_txs(propose_tx_hashes).await;
        if !unknown_hashes.is_empty() {
            let txs = self
                .adapter
                .pull_txs(ctx.clone(), None, unknown_hashes)
                .await?;
            // TODO: concurrently insert
            for tx in txs.into_iter() {
                // Should not handle error here, it is normal that transactions
                // response here are exist in pool.
                let _ = self.insert_tx(ctx.clone(), tx, TxType::ProposeTx).await;
            }
        }
        Ok(())
    }

    fn set_args(&self, timeout_gap: u64, cycles_limit: u64, max_tx_size: u64) {
        self.adapter
            .set_args(timeout_gap, cycles_limit, max_tx_size);
        self.timeout_gap.store(timeout_gap, Ordering::Relaxed);
    }
}

fn check_dup_order_hashes(order_tx_hashes: &[Hash]) -> ProtocolResult<()> {
    let mut dup_set = HashSet::with_capacity(order_tx_hashes.len());

    for hash in order_tx_hashes.iter() {
        if dup_set.contains(hash){
            return Err(MemPoolError::EnsureDup{ hash: hash.clone() }.into())
        }

        dup_set.insert(hash.clone());
    }

    Ok(())
}

pub enum TxType {
    NewTx,
    ProposeTx,
}

#[derive(Debug, Display)]
pub enum MemPoolError {
    #[display(
        fmt = "Tx: {:?} exceeds size limit, now: {}, limit: {} Bytes",
        tx_hash,
        size,
        max_tx_size
    )]
    ExceedSizeLimit {
        tx_hash:     Hash,
        max_tx_size: u64,
        size:        u64,
    },

    #[display(
        fmt = "Tx: {:?} exceeds cycle limit, tx: {}, config: {}",
        tx_hash,
        cycles_limit_tx,
        cycles_limit_config
    )]
    ExceedCyclesLimit {
        tx_hash:             Hash,
        cycles_limit_config: u64,
        cycles_limit_tx:     u64,
    },

    #[display(fmt = "Tx: {:?} inserts failed", tx_hash)]
    Insert { tx_hash: Hash },

    #[display(fmt = "Mempool reaches limit: {}", pool_size)]
    ReachLimit { pool_size: usize },

    #[display(fmt = "Tx: {:?} exists in pool", tx_hash)]
    Dup { tx_hash: Hash },

    #[display(fmt = "Pull txs, require: {}, response: {}", require, response)]
    EnsureBreak { require: usize, response: usize },

    #[display(fmt = "There is duplication in order transactions. duplication tx_hash {:?}", hash)]
    EnsureDup { hash: Hash },

    #[display(fmt = "Fetch full txs, require: {}, response: {}", require, response)]
    MisMatch { require: usize, response: usize },

    #[display(fmt = "Tx inserts candidate_queue failed, len: {}", len)]
    InsertCandidate { len: usize },

    #[display(fmt = "Tx: {:?} check authorization error {:?}", tx_hash, err_info)]
    CheckAuthorization { tx_hash: Hash, err_info: String },

    #[display(fmt = "Check_hash failed, expect: {:?}, get: {:?}", expect, actual)]
    CheckHash { expect: Hash, actual: Hash },

    #[display(fmt = "Tx: {:?} already commit", tx_hash)]
    CommittedTx { tx_hash: Hash },

    #[display(fmt = "Tx: {:?} doesn't match our chain id", tx_hash)]
    WrongChain { tx_hash: Hash },

    #[display(fmt = "Tx: {:?} timeout {}", tx_hash, timeout)]
    Timeout { tx_hash: Hash, timeout: u64 },

    #[display(fmt = "Tx: {:?} invalid timeout", tx_hash)]
    InvalidTimeout { tx_hash: Hash },

    #[display(fmt = "Batch transaction validation failed")]
    VerifyBatchTransactions,

    #[display(fmt = "Encode transaction to JSON failed")]
    EncodeJson,
}

impl Error for MemPoolError {}

impl From<MemPoolError> for ProtocolError {
    fn from(error: MemPoolError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Mempool, Box::new(error))
    }
}
