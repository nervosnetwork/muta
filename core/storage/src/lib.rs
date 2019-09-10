// Remove this clippy bug with async await is resolved.
// ISSUE: https://github.com/rust-lang/rust-clippy/issues/3988
#![allow(clippy::needless_lifetimes)]

pub mod adapter;

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;

use protocol::traits::{
    Storage, StorageAdapter, StorageBatchModify, StorageCategory, StorageSchema,
};
use protocol::types::{Hash, SignedTransaction};
use protocol::ProtocolResult;

#[derive(Debug)]
pub struct ImplStorage<Adapter> {
    adapter: Arc<Adapter>,
}

impl<Adapter: StorageAdapter> ImplStorage<Adapter> {
    pub fn new(adapter: Arc<Adapter>) -> Self {
        Self { adapter }
    }
}

pub struct TransactionSchema;

impl StorageSchema for TransactionSchema {
    type Key = Hash;
    type Value = SignedTransaction;

    fn category() -> StorageCategory {
        StorageCategory::SignedTransaction
    }
}

#[async_trait]
impl<Adapter: StorageAdapter> Storage<Adapter> for ImplStorage<Adapter> {
    async fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()> {
        let mut hashes = Vec::with_capacity(signed_txs.len());

        for _stx in signed_txs.iter() {
            // FIXME: should be stx.hash() later
            let hash = Hash::from_bytes(Bytes::from(vec![]))?;
            hashes.push(hash)
        }

        let batch_insert = signed_txs
            .into_iter()
            .map(StorageBatchModify::Insert)
            .collect::<Vec<_>>();

        self.adapter
            .batch_modify::<TransactionSchema>(hashes, batch_insert)
            .await?;

        Ok(())
    }

    async fn get_transaction_by_hash(
        &self,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>> {
        let opt_stx = self.adapter.get::<TransactionSchema>(tx_hash).await?;

        Ok(opt_stx)
    }
}
