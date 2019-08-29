#![feature(test)]

mod error;
mod map;
mod tx_cache;

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;

use protocol::traits::{Context, MemPool, MemPoolAdapter, MixedTxHashes};
use protocol::types::{Hash, SignedTransaction};
use protocol::ProtocolResult;

use crate::error::MemPoolError;
use crate::map::Map;
use crate::tx_cache::TxCache;

/// Memory pool for caching transactions.
struct HashMemPool<Adapter: MemPoolAdapter> {
    /// Pool size limit.
    pool_size: usize,
    /// A system param limits the life time of an off-chain transaction.
    timeout_gap: u64,
    /// The max cycles accumulated in an `Epoch`.
    cycle_limit: u64,
    /// A structure for caching new transactions and responsible transactions of
    /// propose-sync.
    tx_cache: TxCache,
    /// A structure for caching fresh transactions in order transaction hashes.
    callback_cache: Map<SignedTransaction>,
    /// Supply necessary functions from outer modules.
    adapter: Adapter,
    /// Current epoch_id.
    current_epoch_id: AtomicU64,
}

impl<Adapter> HashMemPool<Adapter>
where
    Adapter: MemPoolAdapter,
{
    #[allow(dead_code)]
    pub fn new(
        pool_size: usize,
        timeout_gap: u64,
        cycle_limit: u64,
        current_epoch_id: u64,
        adapter: Adapter,
    ) -> Self {
        HashMemPool {
            pool_size,
            timeout_gap,
            cycle_limit,
            tx_cache: TxCache::new(pool_size),
            callback_cache: Map::new(pool_size),
            adapter,
            current_epoch_id: AtomicU64::new(current_epoch_id),
        }
    }
}

#[async_trait]
impl<Adapter> MemPool<Adapter> for HashMemPool<Adapter>
where
    Adapter: MemPoolAdapter,
{
    async fn insert(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()> {
        let tx_hash = &tx.tx_hash;
        // 1. check size
        if self.tx_cache.len() >= self.pool_size {
            return Err(MemPoolError::ReachLimit {
                pool_size: self.pool_size,
            }
            .into());
        }
        // 2. check pool exist
        if self.tx_cache.contain(tx_hash) {
            return Err(MemPoolError::Dup {
                tx_hash: tx_hash.clone(),
            }
            .into());
        }

        // 3. check signature
        self.adapter
            .check_signature(ctx.clone(), tx.clone())
            .await?;

        // 4. check transaction
        self.adapter
            .check_transaction(ctx.clone(), tx.clone())
            .await?;

        // 5. check storage exist
        self.adapter
            .check_storage_exist(ctx.clone(), tx_hash.clone())
            .await?;

        // 6. do insert
        self.tx_cache.insert_new_tx(tx.clone())?;

        // 7. network broadcast
        self.adapter.broadcast_tx(ctx.clone(), tx).await?;

        Ok(())
    }

    async fn package(&self, _ctx: Context) -> ProtocolResult<MixedTxHashes> {
        let current_epoch_id = self.current_epoch_id.load(Ordering::SeqCst);
        self.tx_cache.package(
            self.cycle_limit,
            current_epoch_id,
            current_epoch_id + self.timeout_gap,
        )
    }

    async fn flush(&self, _ctx: Context, tx_hashes: Vec<Hash>) -> ProtocolResult<()> {
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
        let unknown_hashes = self.tx_cache.show_unknown(order_tx_hashes);
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
        let unknown_hashes = self.tx_cache.show_unknown(propose_tx_hashes);
        if !unknown_hashes.is_empty() {
            let txs = self.adapter.pull_txs(ctx.clone(), unknown_hashes).await?;
            txs.into_iter().for_each(|tx| {
                // Should not handle error here, it is normal that transactions response here
                // are exist in pool.
                let _ = self.tx_cache.insert_propose_tx(tx);
            });
        }
        Ok(())
    }
}
