#![feature(test)]

#[cfg(test)]
mod tests;

pub mod adapter;

use std::convert::From;
use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use derive_more::{Display, From};
use lazy_static::lazy_static;
use tokio::sync::RwLock;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    Context, Storage, StorageAdapter, StorageBatchModify, StorageCategory, StorageSchema,
};
use protocol::types::{Block, Hash, Proof, Receipt, SignedTransaction};
use protocol::Bytes;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

lazy_static! {
    pub static ref LATEST_BLOCK_KEY: Hash = Hash::digest(Bytes::from("latest_hash"));
    pub static ref LATEST_PROOF_KEY: Hash = Hash::digest(Bytes::from("latest_proof"));
    pub static ref OVERLORD_WAL_KEY: Hash = Hash::digest(Bytes::from("overlord_wal"));
}

#[derive(Debug)]
pub struct ImplStorage<Adapter> {
    adapter: Arc<Adapter>,

    latest_block: RwLock<Option<Block>>,
}

impl<Adapter: StorageAdapter> ImplStorage<Adapter> {
    pub fn new(adapter: Arc<Adapter>) -> Self {
        Self {
            adapter,
            latest_block: RwLock::new(None),
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
impl_storage_schema_for!(BlockSchema, u64, Block, Block);
impl_storage_schema_for!(HashBlockSchema, Hash, u64, Block);
impl_storage_schema_for!(LatestBlockSchema, Hash, Block, Block);
impl_storage_schema_for!(LatestProofSchema, Hash, Proof, Block);
impl_storage_schema_for!(OverlordWalSchema, Hash, Bytes, Wal);

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
    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn insert_transactions(
        &self,
        ctx: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        batch_insert!(self, signed_txs, TransactionSchema);
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn insert_block(&self, ctx: Context, block: Block) -> ProtocolResult<()> {
        let height = block.header.height;
        let block_hash = Hash::digest(block.encode_fixed()?);

        self.adapter
            .insert::<BlockSchema>(height.clone(), block.clone())
            .await?;
        self.adapter
            .insert::<HashBlockSchema>(block_hash, height)
            .await?;
        self.adapter
            .insert::<LatestBlockSchema>(LATEST_BLOCK_KEY.clone(), block.clone())
            .await?;

        self.latest_block.write().await.replace(block);

        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn insert_receipts(&self, ctx: Context, receipts: Vec<Receipt>) -> ProtocolResult<()> {
        batch_insert!(self, receipts, ReceiptSchema);
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn update_latest_proof(&self, ctx: Context, proof: Proof) -> ProtocolResult<()> {
        self.adapter
            .insert::<LatestProofSchema>(LATEST_PROOF_KEY.clone(), proof)
            .await?;
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_transaction_by_hash(
        &self,
        ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<SignedTransaction> {
        let stx = get!(self, tx_hash, TransactionSchema);
        Ok(stx)
    }

    #[muta_apm::derive::tracing_span(kind = "storage", logs = "{'txs_len': 'hashes.len()'}")]
    async fn get_transactions(
        &self,
        ctx: Context,
        hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let stxs = get_batch!(self, hashes, TransactionSchema);
        Ok(stxs)
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_latest_block(&self, ctx: Context) -> ProtocolResult<Block> {
        let opt_block = { self.latest_block.read().await.clone() };

        if let Some(block) = opt_block {
            Ok(block)
        } else {
            Ok(get!(self, LATEST_BLOCK_KEY.clone(), LatestBlockSchema))
        }
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_block_by_height(&self, ctx: Context, height: u64) -> ProtocolResult<Block> {
        let block = get!(self, height, BlockSchema);
        Ok(block)
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_block_by_hash(&self, ctx: Context, block_hash: Hash) -> ProtocolResult<Block> {
        let height = get!(self, block_hash, HashBlockSchema);
        let block = get!(self, height, BlockSchema);
        Ok(block)
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_receipt(&self, ctx: Context, hash: Hash) -> ProtocolResult<Receipt> {
        let receipt = get!(self, hash, ReceiptSchema);
        Ok(receipt)
    }

    #[muta_apm::derive::tracing_span(kind = "storage", logs = "{'receipts_len': 'hashes.len()'}")]
    async fn get_receipts(&self, ctx: Context, hashes: Vec<Hash>) -> ProtocolResult<Vec<Receipt>> {
        let receipts = get_batch!(self, hashes, ReceiptSchema);
        Ok(receipts)
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_latest_proof(&self, ctx: Context) -> ProtocolResult<Proof> {
        let proof = get!(self, LATEST_PROOF_KEY.clone(), LatestProofSchema);
        Ok(proof)
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn update_overlord_wal(&self, ctx: Context, info: Bytes) -> ProtocolResult<()> {
        self.adapter
            .insert::<OverlordWalSchema>(OVERLORD_WAL_KEY.clone(), info)
            .await?;
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn load_overlord_wal(&self, ctx: Context) -> ProtocolResult<Bytes> {
        let wal_info = get!(self, OVERLORD_WAL_KEY.clone(), OverlordWalSchema);
        Ok(wal_info)
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
