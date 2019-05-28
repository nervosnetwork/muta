use core_context::Context;
use core_types::{Block, Hash, SignedTransaction};

use crate::{BoxFuture, FutTxPoolResult, SyncStatus, SynchronizerError};

pub type FutSyncResult<T> = BoxFuture<'static, Result<T, SynchronizerError>>;

pub trait TransactionPool: Clone + Send + Sync {
    fn broadcast_batch(&self, txs: Vec<SignedTransaction>);

    fn pull_txs(&self, ctx: Context, hashes: Vec<Hash>) -> FutTxPoolResult<Vec<SignedTransaction>>;
}

pub trait Consensus: Clone + Send + Sync {
    fn proposal(&self, proposal: Vec<u8>);

    fn vote(&self, vote: Vec<u8>);
}

pub trait Synchronizer: Send + Sync {
    fn broadcast_status(&self, status: SyncStatus);

    fn pull_blocks(&self, ctx: Context, heights: Vec<u64>) -> FutSyncResult<Vec<Block>>;

    fn pull_txs_sync(
        &self,
        ctx: Context,
        tx_hashes: &[Hash],
    ) -> FutSyncResult<Vec<SignedTransaction>>;
}

pub trait PeerCount: Send + Sync {
    fn peer_count(&self) -> usize;
}
