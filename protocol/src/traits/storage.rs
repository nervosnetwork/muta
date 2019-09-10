use async_trait::async_trait;
use futures::stream::Stream;

use crate::codec::ProtocolCodec;
use crate::types::{Hash, SignedTransaction};
use crate::ProtocolResult;

#[derive(Debug, Copy, Clone, Display)]
pub enum StorageCategory {
    Epoch,
    Receipt,
    SignedTransaction,
}

pub trait StorageSchema {
    type Key: ProtocolCodec + Send;
    type Value: ProtocolCodec + Send;

    fn category() -> StorageCategory;
}

#[async_trait]
pub trait Storage<Adapter: StorageAdapter>: Send + Sync {
    async fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()>;

    async fn get_transaction_by_hash(
        &self,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>>;
}

pub enum StorageBatchModify<S: StorageSchema> {
    Remove,
    Insert(<S as StorageSchema>::Value),
}

#[async_trait]
pub trait StorageAdapter: Send + Sync {
    async fn insert<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
        val: <S as StorageSchema>::Value,
    ) -> ProtocolResult<()>;

    async fn get<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<Option<<S as StorageSchema>::Value>>;

    async fn remove<S: StorageSchema>(&self, key: <S as StorageSchema>::Key) -> ProtocolResult<()>;

    async fn contains<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<bool>;

    // TODO: Query struct?
    fn iter<S: StorageSchema + 'static>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
    ) -> Box<dyn Stream<Item = ProtocolResult<Option<<S as StorageSchema>::Value>>>>;

    async fn batch_modify<S: StorageSchema>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
        vals: Vec<StorageBatchModify<S>>,
    ) -> ProtocolResult<()>;
}
