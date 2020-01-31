#![feature(test)]

#[cfg(test)]
mod tests;

pub mod adapter;

use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use derive_more::{Display, From};
use lazy_static::lazy_static;
use tokio::sync::RwLock;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    Storage, StorageAdapter, StorageBatchModify, StorageCategory, StorageSchema,
};
use protocol::types::{Block, Hash, Proof, Receipt, SignedTransaction, WalSaveTxs};
use protocol::Bytes;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

lazy_static! {
    pub static ref LATEST_EPOCH_KEY: Hash = Hash::digest(Bytes::from("latest_hash"));
    pub static ref LATEST_PROOF_KEY: Hash = Hash::digest(Bytes::from("latest_proof"));
    pub static ref OVERLORD_WAL_KEY: Hash = Hash::digest(Bytes::from("overlord_wal"));
    pub static ref MUTA_WAL_KEY: Hash = Hash::digest(Bytes::from("muta_wal"));
    pub static ref EXEC_QUEUE_WAL_KEY: Hash = Hash::digest(Bytes::from("exec_quequ_wal"));
}

#[derive(Debug)]
pub struct ImplStorage<Adapter> {
    adapter: Arc<Adapter>,

    latest_epoch: RwLock<Option<Block>>,
}

impl<Adapter: StorageAdapter> ImplStorage<Adapter> {
    pub fn new(adapter: Arc<Adapter>) -> Self {
        Self {
            adapter,
            latest_epoch: RwLock::new(None),
        }
    }
}

macro_rules! impl_storage_schema_for {
    ($name: ident, $key: ident, $val: ident, $category: ident) => {
        pub struct $name;

        impl StorageSchema for $name {
            type Key = $key;
            type Value = $val;

            fn category() -> StorageCategory {
                StorageCategory::$category
            }
        }
    };
}

impl_storage_schema_for!(
    TransactionSchema,
    Hash,
    SignedTransaction,
    SignedTransaction
);
impl_storage_schema_for!(ReceiptSchema, Hash, Receipt, Receipt);
impl_storage_schema_for!(EpochSchema, u64, Block, Block);
impl_storage_schema_for!(HashEpochSchema, Hash, u64, Block);
impl_storage_schema_for!(LatestEpochSchema, Hash, Block, Block);
impl_storage_schema_for!(LatestProofSchema, Hash, Proof, Block);
impl_storage_schema_for!(OverlordWalSchema, Hash, Bytes, Wal);
impl_storage_schema_for!(MutaWalSchema, Hash, Bytes, Wal);
impl_storage_schema_for!(ExecQueueWalSchema, Hash, Bytes, Wal);
impl_storage_schema_for!(WalTransactionSchema, Hash, WalSaveTxs, Wal);

macro_rules! batch_insert {
    ($self_: ident,$vec: expr, $schema: ident) => {
        let mut hashes = Vec::with_capacity($vec.len());

        for item in $vec.iter() {
            hashes.push(item.tx_hash.clone())
        }

        let batch_insert = $vec
            .into_iter()
            .map(StorageBatchModify::Insert)
            .collect::<Vec<_>>();

        $self_
            .adapter
            .batch_modify::<$schema>(hashes, batch_insert)
            .await?;
    };
}

macro_rules! get_batch {
    ($self_: ident, $keys: expr, $schema: ident) => {{
        let opt = $self_.adapter.get_batch::<$schema>($keys).await?;
        opts_to_flat(opt)
    }};
}

macro_rules! get {
    ($self_: ident, $key: expr, $schema: ident) => {{
        let opt = $self_.adapter.get::<$schema>($key).await?;
        check_none(opt)?
    }};
}

#[async_trait]
impl<Adapter: StorageAdapter> Storage for ImplStorage<Adapter> {
    async fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()> {
        batch_insert!(self, signed_txs, TransactionSchema);
        Ok(())
    }

    async fn insert_epoch(&self, block: Block) -> ProtocolResult<()> {
        let height = block.header.height;
        let epoch_hash = Hash::digest(block.encode_fixed()?);

        self.adapter
            .insert::<EpochSchema>(height.clone(), block.clone())
            .await?;
        self.adapter
            .insert::<HashEpochSchema>(epoch_hash, height)
            .await?;
        self.adapter
            .insert::<LatestEpochSchema>(LATEST_EPOCH_KEY.clone(), block.clone())
            .await?;

        self.latest_epoch.write().await.replace(block);

        Ok(())
    }

    async fn insert_receipts(&self, receipts: Vec<Receipt>) -> ProtocolResult<()> {
        batch_insert!(self, receipts, ReceiptSchema);
        Ok(())
    }

    async fn update_latest_proof(&self, proof: Proof) -> ProtocolResult<()> {
        self.adapter
            .insert::<LatestProofSchema>(LATEST_PROOF_KEY.clone(), proof)
            .await?;
        Ok(())
    }

    async fn get_transaction_by_hash(&self, tx_hash: Hash) -> ProtocolResult<SignedTransaction> {
        let stx = get!(self, tx_hash, TransactionSchema);
        Ok(stx)
    }

    async fn get_transactions(&self, hashes: Vec<Hash>) -> ProtocolResult<Vec<SignedTransaction>> {
        let stxs = get_batch!(self, hashes, TransactionSchema);
        Ok(stxs)
    }

    async fn get_latest_epoch(&self) -> ProtocolResult<Block> {
        let opt_epoch = { self.latest_epoch.read().await.clone() };

        if let Some(block) = opt_epoch {
            Ok(block)
        } else {
            Ok(get!(self, LATEST_EPOCH_KEY.clone(), LatestEpochSchema))
        }
    }

    async fn get_epoch_by_epoch_id(&self, height: u64) -> ProtocolResult<Block> {
        let block = get!(self, height, EpochSchema);
        Ok(block)
    }

    async fn get_epoch_by_hash(&self, epoch_hash: Hash) -> ProtocolResult<Block> {
        let height = get!(self, epoch_hash, HashEpochSchema);
        let block = get!(self, height, EpochSchema);
        Ok(block)
    }

    async fn get_receipt(&self, hash: Hash) -> ProtocolResult<Receipt> {
        let receipt = get!(self, hash, ReceiptSchema);
        Ok(receipt)
    }

    async fn get_receipts(&self, hashes: Vec<Hash>) -> ProtocolResult<Vec<Receipt>> {
        let receipts = get_batch!(self, hashes, ReceiptSchema);
        Ok(receipts)
    }

    async fn get_latest_proof(&self) -> ProtocolResult<Proof> {
        let proof = get!(self, LATEST_PROOF_KEY.clone(), LatestProofSchema);
        Ok(proof)
    }

    async fn update_overlord_wal(&self, info: Bytes) -> ProtocolResult<()> {
        self.adapter
            .insert::<OverlordWalSchema>(OVERLORD_WAL_KEY.clone(), info)
            .await?;
        Ok(())
    }

    async fn update_muta_wal(&self, info: Bytes) -> ProtocolResult<()> {
        self.adapter
            .insert::<MutaWalSchema>(MUTA_WAL_KEY.clone(), info)
            .await?;
        Ok(())
    }

    async fn load_overlord_wal(&self) -> ProtocolResult<Bytes> {
        let wal_info = get!(self, OVERLORD_WAL_KEY.clone(), OverlordWalSchema);
        Ok(wal_info)
    }

    async fn load_muta_wal(&self) -> ProtocolResult<Bytes> {
        let wal_info = get!(self, MUTA_WAL_KEY.clone(), MutaWalSchema);
        Ok(wal_info)
    }

    async fn update_exec_queue_wal(&self, info: Bytes) -> ProtocolResult<()> {
        self.adapter
            .insert::<ExecQueueWalSchema>(EXEC_QUEUE_WAL_KEY.clone(), info)
            .await?;
        Ok(())
    }

    async fn load_exec_queue_wal(&self) -> ProtocolResult<Bytes> {
        let wal_info = get!(self, EXEC_QUEUE_WAL_KEY.clone(), ExecQueueWalSchema);
        Ok(wal_info)
    }

    async fn insert_wal_transactions(
        &self,
        epoch_hash: Hash,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let wal_txs_info = WalSaveTxs { inner: signed_txs };
        self.adapter
            .insert::<WalTransactionSchema>(epoch_hash, wal_txs_info)
            .await?;
        Ok(())
    }

    async fn get_wal_transactions(
        &self,
        epoch_hash: Hash,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let stxs = get!(self, epoch_hash, WalTransactionSchema);
        Ok(stxs.inner)
    }
}

fn opts_to_flat<T>(values: Vec<Option<T>>) -> Vec<T> {
    values
        .into_iter()
        .filter(Option::is_some)
        .map(|v| v.expect("get value"))
        .collect()
}

fn check_none<T>(opt: Option<T>) -> ProtocolResult<T> {
    opt.ok_or_else(|| StorageError::GetNone.into())
}

#[derive(Debug, Display, From)]
pub enum StorageError {
    #[display(fmt = "get none")]
    GetNone,
}

impl Error for StorageError {}

impl From<StorageError> for ProtocolError {
    fn from(err: StorageError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
    }
}
