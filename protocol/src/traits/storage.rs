use async_trait::async_trait;
use bytes::Bytes;

use crate::types::{Hash, SignedTransaction};
use crate::ProtocolResult;

#[derive(Debug, Copy, Clone)]
pub enum StorageCategory {
    Epoch,
    Receipt,
    SignedTransaction,
}

#[async_trait]
pub trait Storage<Adapter: StorageAdapter>: Send + Sync {
    async fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()>;

    async fn get_transaction_by_hash(
        &self,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>>;
}

#[async_trait]
pub trait StorageAdapter: Send + Sync {
    async fn get(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<Option<Bytes>>;

    async fn get_batch(
        &self,
        c: StorageCategory,
        keys: Vec<Bytes>,
    ) -> ProtocolResult<Vec<Option<Bytes>>>;

    async fn insert(&self, c: StorageCategory, key: Bytes, value: Bytes) -> ProtocolResult<()>;

    async fn insert_batch(
        &self,
        c: StorageCategory,
        keys: Vec<Bytes>,
        values: Vec<Bytes>,
    ) -> ProtocolResult<()>;

    async fn contains(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<bool>;

    async fn remove(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<()>;

    async fn remove_batch(&self, c: StorageCategory, keys: Vec<Bytes>) -> ProtocolResult<()>;
}
