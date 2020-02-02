use async_trait::async_trait;
use creep::Context;

use crate::types::{Hash, SignedTransaction};
use crate::ProtocolResult;

#[allow(dead_code)]
pub struct MixedTxHashes {
    pub order_tx_hashes:   Vec<Hash>,
    pub propose_tx_hashes: Vec<Hash>,
}

impl MixedTxHashes {
    pub fn clap(self) -> (Vec<Hash>, Vec<Hash>) {
        (self.order_tx_hashes, self.propose_tx_hashes)
    }
}

#[async_trait]
pub trait MemPool: Send + Sync {
    async fn insert(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()>;

    async fn package(&self, ctx: Context, cycle_limit: u64) -> ProtocolResult<MixedTxHashes>;

    async fn flush(&self, ctx: Context, tx_hashes: Vec<Hash>) -> ProtocolResult<()>;

    async fn get_full_txs(
        &self,
        ctx: Context,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    async fn ensure_order_txs(
        &self,
        ctx: Context,
        order_tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<()>;

    async fn sync_propose_txs(
        &self,
        ctx: Context,
        propose_tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<()>;
}

#[async_trait]
pub trait MemPoolAdapter: Send + Sync {
    async fn pull_txs(
        &self,
        ctx: Context,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    async fn broadcast_tx(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()>;

    async fn check_signature(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()>;

    async fn check_transaction(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()>;

    async fn check_storage_exist(&self, ctx: Context, tx_hash: Hash) -> ProtocolResult<()>;

    async fn get_latest_height(&self, ctx: Context) -> ProtocolResult<u64>;

    fn set_timeout_gap(&self, timeout_gap: u64);
}
