// Remove this clippy bug with async await is resolved.
// ISSUE: https://github.com/rust-lang/rust-clippy/issues/3988
#![allow(clippy::needless_lifetimes)]

pub mod adapter;

use std::sync::Arc;

use async_trait::async_trait;

use protocol::traits::{Storage, StorageAdapter, StorageCategory};
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

#[async_trait]
impl<Adapter: StorageAdapter> Storage<Adapter> for ImplStorage<Adapter> {
    async fn insert_transactions(&self, _signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()> {
        self.adapter
            .insert_batch(StorageCategory::SignedTransaction, vec![], vec![])
            .await?;
        Ok(())
    }

    async fn get_transaction_by_hash(
        &self,
        _tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>> {
        unimplemented!();
        // let adapter = Arc::clone(&self.adapter);
        //
        // async move {
        //     adapter.get(tx_hash.as_bytes()).await.unwrap();
        // }
    }
}
