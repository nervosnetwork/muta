use async_trait::async_trait;

use crate::traits::{Context, ExecResp};
use crate::types::{Address, Epoch, Hash, Receipt, SignedTransaction};
use crate::ProtocolResult;

#[async_trait]
pub trait APIAdapter: Send + Sync {
    async fn insert_signed_txs(
        &self,
        ctx: Context,
        signed_tx: SignedTransaction,
    ) -> ProtocolResult<()>;

    async fn get_epoch_by_id(&self, ctx: Context, epoch_id: Option<u64>) -> ProtocolResult<Epoch>;

    async fn get_receipt_by_tx_hash(&self, ctx: Context, tx_hash: Hash) -> ProtocolResult<Receipt>;

    async fn get_transaction_by_hash(
        &self,
        ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<SignedTransaction>;

    async fn query_service(
        &self,
        ctx: Context,
        epoch_id: u64,
        cycles_limit: u64,
        cycles_price: u64,
        caller: Address,
        service_name: String,
        method: String,
        payload: String,
    ) -> ProtocolResult<ExecResp>;
}
