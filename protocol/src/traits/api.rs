use async_trait::async_trait;

use crate::traits::Context;
use crate::types::{Address, Balance, Epoch, Hash, Receipt, SignedTransaction};
use crate::ProtocolResult;

#[async_trait]
pub trait APIAdapter: Send + Sync {
    async fn insert_signed_txs(
        &self,
        ctx: Context,
        signed_tx: SignedTransaction,
    ) -> ProtocolResult<()>;

    async fn get_latest_epoch(&self, ctx: Context) -> ProtocolResult<Epoch>;

    async fn get_epoch_by_id(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch>;

    async fn get_receipt_by_tx_hash(&self, ctx: Context, tx_hash: Hash) -> ProtocolResult<Receipt>;

    async fn get_balance(&self, ctx: Context, address: &Address) -> ProtocolResult<Balance>;
}
