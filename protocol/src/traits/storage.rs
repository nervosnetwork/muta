use async_trait::async_trait;
use derive_more::Display;

use crate::codec::ProtocolCodec;
use crate::types::epoch::{Epoch, Proof};
use crate::types::receipt::Receipt;
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

    async fn insert_epoch(&self, epoch: Epoch) -> ProtocolResult<()>;

    async fn insert_receipts(&self, receipts: Vec<Receipt>) -> ProtocolResult<()>;

    async fn update_latest_proof(&self, proof: Proof) -> ProtocolResult<()>;

    async fn get_transaction_by_hash(&self, tx_hash: Hash) -> ProtocolResult<SignedTransaction>;

    async fn get_transactions(&self, hashes: Vec<Hash>) -> ProtocolResult<Vec<SignedTransaction>>;

    async fn get_latest_epoch(&self) -> ProtocolResult<Epoch>;

    async fn get_epoch_by_epoch_id(&self, epoch_id: u64) -> ProtocolResult<Epoch>;

    async fn get_epoch_by_hash(&self, epoch_hash: Hash) -> ProtocolResult<Epoch>;

    async fn get_receipt(&self, hash: Hash) -> ProtocolResult<Receipt>;

    async fn get_receipts(&self, hash: Vec<Hash>) -> ProtocolResult<Vec<Receipt>>;

    async fn get_latest_proof(&self) -> ProtocolResult<Proof>;
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

    async fn get_batch<S: StorageSchema>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
    ) -> ProtocolResult<Vec<Option<<S as StorageSchema>::Value>>> {
        let mut vec = Vec::new();

        for key in keys {
            vec.push(self.get::<S>(key).await?);
        }

        Ok(vec)
    }

    async fn remove<S: StorageSchema>(&self, key: <S as StorageSchema>::Key) -> ProtocolResult<()>;

    async fn contains<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<bool>;

    async fn batch_modify<S: StorageSchema>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
        vals: Vec<StorageBatchModify<S>>,
    ) -> ProtocolResult<()>;
}
