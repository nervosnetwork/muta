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

    async fn package(
        &self,
        ctx: Context,
        cycles_limit: u64,
        tx_num_limit: u64,
    ) -> ProtocolResult<MixedTxHashes>;

    async fn flush(&self, ctx: Context, tx_hashes: &[Hash]) -> ProtocolResult<()>;

    async fn get_full_txs(
        &self,
        ctx: Context,
        height: Option<u64>,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    async fn ensure_order_txs(
        &self,
        ctx: Context,
        height: Option<u64>,
        order_tx_hashes: &[Hash],
    ) -> ProtocolResult<()>;

    async fn sync_propose_txs(
        &self,
        ctx: Context,
        propose_tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<()>;

    fn set_args(&self, timeout_gap: u64, cycles_limit: u64, max_tx_size: u64);
}

#[async_trait]
pub trait MemPoolAdapter: Send + Sync {
    async fn pull_txs(
        &self,
        ctx: Context,
        height: Option<u64>,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    async fn broadcast_tx(&self, ctx: Context, tx: SignedTransaction) -> ProtocolResult<()>;

    async fn check_authorization(
        &self,
        ctx: Context,
        tx: Box<SignedTransaction>,
    ) -> ProtocolResult<()>;

    async fn check_transaction(&self, ctx: Context, tx: &SignedTransaction) -> ProtocolResult<()>;

    async fn check_storage_exist(&self, ctx: Context, tx_hash: &Hash) -> ProtocolResult<()>;

    async fn get_latest_height(&self, ctx: Context) -> ProtocolResult<u64>;

    async fn get_transactions_from_storage(
        &self,
        ctx: Context,
        block_height: Option<u64>,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<Option<SignedTransaction>>>;

    fn report_good(&self, ctx: Context);

    fn set_args(&self, timeout_gap: u64, cycles_limit: u64, max_tx_size: u64);
}
