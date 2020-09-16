use async_trait::async_trait;

use crate::traits::{Context, ServiceResponse};
use crate::types::{Address, Block, BlockHeader, Hash, Receipt, SignedTransaction};
use crate::ProtocolResult;

#[async_trait]
pub trait APIAdapter: Send + Sync {
    async fn insert_signed_txs(
        &self,
        ctx: Context,
        signed_tx: SignedTransaction,
    ) -> ProtocolResult<()>;

    async fn get_block_by_height(
        &self,
        ctx: Context,
        height: Option<u64>,
    ) -> ProtocolResult<Option<Block>>;

    async fn get_block_header_by_height(
        &self,
        ctx: Context,
        height: Option<u64>,
    ) -> ProtocolResult<Option<BlockHeader>>;

    async fn get_receipt_by_tx_hash(
        &self,
        ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<Receipt>>;

    async fn get_transaction_by_hash(
        &self,
        ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>>;

    async fn query_service(
        &self,
        ctx: Context,
        height: u64,
        cycles_limit: u64,
        cycles_price: u64,
        caller: Address,
        service_name: String,
        method: String,
        payload: String,
    ) -> ProtocolResult<ServiceResponse<String>>;
}
