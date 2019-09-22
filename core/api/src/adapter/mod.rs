use std::sync::Arc;

use async_trait::async_trait;

use protocol::traits::{APIAdapter, Context, MemPool, Storage};
use protocol::types::{Address, Balance, Epoch, Hash, Receipt, SignedTransaction};
use protocol::ProtocolResult;

pub struct DefaultAPIAdapter<M, S> {
    mempool: Arc<M>,
    storage: Arc<S>,
}

#[async_trait]
impl<M: MemPool, S: Storage> APIAdapter for DefaultAPIAdapter<M, S> {
    async fn insert_signed_txs(
        &self,
        ctx: Context,
        signed_tx: SignedTransaction,
    ) -> ProtocolResult<()> {
        self.mempool.insert(ctx, signed_tx).await
    }

    async fn get_latest_epoch(&self, _ctx: Context) -> ProtocolResult<Epoch> {
        self.storage.get_latest_epoch().await
    }

    async fn get_epoch_by_id(&self, _ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        self.storage.get_epoch_by_epoch_id(epoch_id).await
    }

    async fn get_receipt_by_tx_hash(
        &self,
        _ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<Receipt> {
        self.storage.get_receipt(tx_hash).await
    }

    async fn get_balance(&self, _ctx: Context, _address: &Address) -> ProtocolResult<Balance> {
        Ok(Balance::from(0u64))
    }
}
