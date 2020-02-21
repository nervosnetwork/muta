#![feature(test)]

mod adapter;
mod context;
mod map;
#[cfg(test)]
mod tests;
mod tx_cache;

pub use adapter::message::{
    MsgPushTxs, NewTxsHandler, PullTxsHandler, END_GOSSIP_NEW_TXS, RPC_PULL_TXS, RPC_RESP_PULL_TXS,
};
pub use adapter::DefaultMemPoolAdapter;
pub use adapter::{DEFAULT_BROADCAST_TXS_INTERVAL, DEFAULT_BROADCAST_TXS_SIZE};

use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use derive_more::Display;

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
    callback_cache: Map<SignedTransaction>,
    /// Supply necessary functions from outer modules.
    adapter:        Adapter,
}

impl<Adapter> HashMemPool<Adapter>
where
    Adapter: MemPoolAdapter,
{
    pub fn new(pool_size: usize, adapter: Adapter) -> Self {
        HashMemPool {
            pool_size,
            timeout_gap: AtomicU64::new(0),
            tx_cache: TxCache::new(pool_size * 2),
            callback_cache: Map::new(pool_size),
            adapter,
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

    fn show_unknown_txs(&self, tx_hashes: Vec<Hash>) -> Vec<Hash> {
        self.tx_cache
            .show_unknown(tx_hashes)
            .into_iter()
            .filter(|tx_hash| !self.callback_cache.contains_key(tx_hash))
            .collect()
    }
}

#[async_trait]
impl<Adapter> MemPool for HashMemPool<Adapter>
where
    Adapter: MemPoolAdapter,
{
    async fn insert(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()> {
        let tx_hash = &tx.tx_hash;

        self.tx_cache.check_reach_limit(self.pool_size)?;
        self.tx_cache.check_exist(tx_hash)?;
        self.adapter
            .check_signature(ctx.clone(), tx.clone())
            .await?;
        self.adapter
            .check_transaction(ctx.clone(), tx.clone())
            .await?;
        self.adapter
            .check_storage_exist(ctx.clone(), tx_hash.clone())
            .await?;
        self.tx_cache.insert_new_tx(tx.clone())?;

        if !ctx.is_network_origin_txs() {
            self.adapter.broadcast_tx(ctx, tx).await?;
        }

        Ok(())
    }

    async fn package(&self, ctx: Context, cycle_limit: u64) -> ProtocolResult<MixedTxHashes> {
        let current_height = self.adapter.get_latest_height(ctx.clone()).await?;
        log::info!(
            "[mempool]: {:?} txs in map and {:?} txs in queue while package",
            self.tx_cache.len(),
            self.tx_cache.queue_len(),
        );
        self.tx_cache.package(
            cycle_limit,
            current_height,
            current_height + self.timeout_gap.load(Ordering::Relaxed),
        )
    }

    async fn flush(&self, _ctx: Context, tx_hashes: Vec<Hash>) -> ProtocolResult<()> {
        log::info!(
            "[mempool]: flush mempool with {:?} tx_hashes",
            tx_hashes.len(),
        );
        self.tx_cache.flush(&tx_hashes);
        self.callback_cache.clear();
        Ok(())
    }

    async fn get_full_txs(
        &self,
        _ctx: Context,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let len = tx_hashes.len();
        let mut full_txs = Vec::with_capacity(len);

        for tx_hash in tx_hashes {
            if let Some(tx) = self.tx_cache.get(&tx_hash) {
                full_txs.push(tx);
            } else if let Some(tx) = self.callback_cache.get(&tx_hash) {
                full_txs.push(tx);
            }
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

    async fn ensure_order_txs(
        &self,
        ctx: Context,
        order_tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<()> {
        let unknown_hashes = self.show_unknown_txs(order_tx_hashes);
        if !unknown_hashes.is_empty() {
            let unknown_len = unknown_hashes.len();
            let txs = self.adapter.pull_txs(ctx.clone(), unknown_hashes).await?;
            // Make sure response signed_txs is the same size of request hashes.
            if txs.len() != unknown_len {
                return Err(MemPoolError::EnsureBreak {
                    require:  unknown_len,
                    response: txs.len(),
                }
                .into());
            }
            txs.into_iter().for_each(|tx| {
                self.callback_cache.insert(tx.tx_hash.clone(), tx);
            });
        }

        Ok(())
    }

    async fn sync_propose_txs(
        &self,
        ctx: Context,
        propose_tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<()> {
        let unknown_hashes = self.show_unknown_txs(propose_tx_hashes);
        if !unknown_hashes.is_empty() {
            let txs = self.adapter.pull_txs(ctx.clone(), unknown_hashes).await?;
            // TODO: concurrently insert
            for tx in txs.into_iter() {
                // Should not handle error here, it is normal that transactions
                // response here are exist in pool.
                let _ = self.insert(ctx.clone(), tx).await;
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

    #[display(fmt = "Fetch full txs, require: {}, response: {}", require, response)]
    MisMatch { require: usize, response: usize },

    #[display(fmt = "Tx inserts candidate_queue failed, len: {}", len)]
    InsertCandidate { len: usize },

    #[display(fmt = "Tx: {:?} check_sig failed", tx_hash)]
    CheckSig { tx_hash: Hash },

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
}

impl Error for MemPoolError {}

impl From<MemPoolError> for ProtocolError {
    fn from(error: MemPoolError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Mempool, Box::new(error))
    }
}
