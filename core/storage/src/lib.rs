#![feature(test)]

#[cfg(test)]
mod tests;

pub mod adapter;

use std::collections::{HashMap, HashSet};
use std::convert::From;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use derive_more::{Display, From};
use lazy_static::lazy_static;
use tokio::sync::RwLock;

use common_apm::muta_apm;

use protocol::codec::ProtocolCodecSync;
use protocol::traits::{
    Context, Storage, StorageAdapter, StorageBatchModify, StorageCategory, StorageSchema,
};
use protocol::types::{Block, Hash, Proof, Receipt, SignedTransaction};
use protocol::Bytes;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

const BATCH_VALUE_DECODE_NUMBER: usize = 1000;

lazy_static! {
    pub static ref LATEST_BLOCK_KEY: Hash = Hash::digest(Bytes::from("latest_hash"));
    pub static ref LATEST_PROOF_KEY: Hash = Hash::digest(Bytes::from("latest_proof"));
    pub static ref OVERLORD_WAL_KEY: Hash = Hash::digest(Bytes::from("overlord_wal"));
}

// FIXME: https://github.com/facebook/rocksdb/wiki/Transactions
macro_rules! batch_insert {
    ($self_: ident, $block_height:expr, $vec: expr, $schema: ident) => {
        let (hashes, heights) = $vec
            .iter()
            .map(|item| {
                (
                    item.tx_hash.clone(),
                    StorageBatchModify::Insert($block_height),
                )
            })
            .unzip();

        let (keys, batch_stxs): (Vec<_>, Vec<_>) = $vec
            .into_iter()
            .map(|item| {
                (
                    CommonHashKey::new($block_height, item.tx_hash.clone()),
                    StorageBatchModify::Insert(item),
                )
            })
            .unzip();

        $self_
            .adapter
            .batch_modify::<$schema>(keys, batch_stxs)
            .await?;

        $self_
            .adapter
            .batch_modify::<HashHeightSchema>(hashes, heights)
            .await?;
    };
}

macro_rules! get {
    ($self_: ident, $key: expr, $schema: ident) => {{
        $self_.adapter.get::<$schema>($key).await
    }};
}

macro_rules! ensure_get {
    ($self_: ident, $key: expr, $schema: ident) => {{
        let opt = get!($self_, $key, $schema)?;
        opt.ok_or_else(|| StorageError::GetNone)?
    }};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommonPrefix {
    block_height: [u8; 8], // BigEndian
}

impl CommonPrefix {
    pub fn new(block_height: u64) -> Self {
        CommonPrefix {
            block_height: block_height.to_be_bytes(),
        }
    }

    pub fn len() -> usize {
        8
    }

    pub fn height(self) -> u64 {
        u64::from_be_bytes(self.block_height)
    }

    pub fn make_hash_key(self, hash: &Hash) -> [u8; 40] {
        debug_assert!(hash.as_bytes().len() == 32);

        let mut key = [0u8; 40];
        key[0..8].copy_from_slice(&self.block_height);
        key[8..40].copy_from_slice(&hash.as_bytes()[..32]);

        key
    }
}

impl AsRef<[u8]> for CommonPrefix {
    fn as_ref(&self) -> &[u8] {
        &self.block_height
    }
}

impl From<&[u8]> for CommonPrefix {
    fn from(bytes: &[u8]) -> CommonPrefix {
        debug_assert!(bytes.len() >= 8);

        let mut h_buf = [0u8; 8];
        h_buf.copy_from_slice(&bytes[0..8]);

        CommonPrefix {
            block_height: h_buf,
        }
    }
}

impl ProtocolCodecSync for CommonPrefix {
    fn encode_sync(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::copy_from_slice(&self.block_height))
    }

    fn decode_sync(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(CommonPrefix::from(&bytes[..8]))
    }
}

#[derive(Debug, Clone)]
pub struct CommonHashKey {
    prefix: CommonPrefix,
    hash:   Hash,
}

impl CommonHashKey {
    pub fn new(block_height: u64, hash: Hash) -> Self {
        CommonHashKey {
            prefix: CommonPrefix::new(block_height),
            hash,
        }
    }

    pub fn height(&self) -> u64 {
        self.prefix.height()
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }
}

impl ProtocolCodecSync for CommonHashKey {
    fn encode_sync(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::copy_from_slice(
            &self.prefix.make_hash_key(&self.hash),
        ))
    }

    fn decode_sync(mut bytes: Bytes) -> ProtocolResult<Self> {
        debug_assert!(bytes.len() >= CommonPrefix::len());

        let prefix = CommonPrefix::from(&bytes[0..CommonPrefix::len()]);
        let hash = Hash::from_bytes(bytes.split_off(CommonPrefix::len()))?;

        Ok(CommonHashKey { prefix, hash })
    }
}

impl ToString for CommonHashKey {
    fn to_string(&self) -> String {
        format!("{}:{}", self.prefix.height(), self.hash.as_hex())
    }
}

impl FromStr for CommonHashKey {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split(':').collect::<Vec<_>>();
        debug_assert!(parts.len() == 2);

        let height = parts[0].parse::<u64>().map_err(|_| ())?;
        let hash = Hash::from_hex(parts[1]).map_err(|_| ())?;

        Ok(CommonHashKey::new(height, hash))
    }
}

pub type BlockKey = CommonPrefix;

impl_storage_schema_for!(
    TransactionSchema,
    CommonHashKey,
    SignedTransaction,
    SignedTransaction
);
impl_storage_schema_for!(
    TransactionBytesSchema,
    CommonHashKey,
    Bytes,
    SignedTransaction
);
impl_storage_schema_for!(BlockSchema, BlockKey, Block, Block);
impl_storage_schema_for!(ReceiptSchema, CommonHashKey, Receipt, Receipt);
impl_storage_schema_for!(HashHeightSchema, Hash, u64, HashHeight);
impl_storage_schema_for!(LatestBlockSchema, Hash, Block, Block);
impl_storage_schema_for!(LatestProofSchema, Hash, Proof, Block);
impl_storage_schema_for!(OverlordWalSchema, Hash, Bytes, Wal);

#[async_trait]
impl<Adapter: StorageAdapter> Storage for ImplStorage<Adapter> {
    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn insert_transactions(
        &self,
        ctx: Context,
        block_height: u64,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        batch_insert!(self, block_height, signed_txs, TransactionSchema);

        Ok(())
    }

    async fn get_transactions(
        &self,
        _ctx: Context,
        block_height: u64,
        hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<Option<SignedTransaction>>> {
        let key_prefix = CommonPrefix::new(block_height);
        let mut found = Vec::with_capacity(hashes.len());

        {
            let prepare_iter = self
                .adapter
                .prepare_iter::<TransactionBytesSchema, _>(&key_prefix)?;
            let mut iter = prepare_iter.ref_to_iter();

            let set = hashes.iter().collect::<HashSet<_>>();
            let mut count = hashes.len();

            while count > 0 {
                let (key, stx_bytes) = match iter.next() {
                    None => break,
                    Some(Ok(key_to_stx_bytes)) => key_to_stx_bytes,
                    Some(Err(err)) => return Err(err),
                };

                if key.height() != block_height {
                    break;
                }

                if set.contains(&key.hash) {
                    found.push((key.hash, stx_bytes));
                    count -= 1;
                }
            }
        }

        let mut found = {
            if found.len() <= BATCH_VALUE_DECODE_NUMBER {
                found
                    .drain(..)
                    .map(|(k, v): (Hash, Bytes)| SignedTransaction::decode_sync(v).map(|v| (k, v)))
                    .collect::<ProtocolResult<Vec<_>>>()?
                    .into_iter()
                    .collect::<HashMap<_, _>>()
            } else {
                let futs = found
                    .chunks(BATCH_VALUE_DECODE_NUMBER)
                    .map(|vals| {
                        let vals = vals.to_owned();

                        // FIXME: cancel decode
                        tokio::spawn(async move {
                            vals.into_iter()
                                .map(|(k, v)| <_>::decode_sync(v).map(|v| (k, v)))
                                .collect::<ProtocolResult<Vec<_>>>()
                        })
                    })
                    .collect::<Vec<_>>();

                futures::future::try_join_all(futs)
                    .await
                    .map_err(|_| StorageError::BatchDecode)?
                    .into_iter()
                    .collect::<ProtocolResult<Vec<Vec<_>>>>()?
                    .into_iter()
                    .flatten()
                    .collect::<HashMap<_, _>>()
            }
        };

        Ok(hashes
            .into_iter()
            .map(|h| found.remove(&h))
            .collect::<Vec<_>>())
    }

    async fn get_transaction_by_hash(
        &self,
        _ctx: Context,
        hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>> {
        if let Some(block_height) = get!(self, hash.clone(), HashHeightSchema)? {
            get!(
                self,
                CommonHashKey::new(block_height, hash),
                TransactionSchema
            )
        } else {
            Ok(None)
        }
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn insert_block(&self, ctx: Context, block: Block) -> ProtocolResult<()> {
        self.adapter
            .insert::<BlockSchema>(BlockKey::new(block.header.height), block.clone())
            .await?;
        self.adapter
            .insert::<LatestBlockSchema>(LATEST_BLOCK_KEY.clone(), block.clone())
            .await?;

        self.latest_block.write().await.replace(block);

        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_block(&self, ctx: Context, height: u64) -> ProtocolResult<Option<Block>> {
        self.adapter.get::<BlockSchema>(BlockKey::new(height)).await
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn insert_receipts(
        &self,
        ctx: Context,
        block_height: u64,
        receipts: Vec<Receipt>,
    ) -> ProtocolResult<()> {
        batch_insert!(self, block_height, receipts, ReceiptSchema);

        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_receipts(
        &self,
        ctx: Context,
        block_height: u64,
        hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<Option<Receipt>>> {
        let key_prefix = CommonPrefix::new(block_height);
        let prepare_iter = self.adapter.prepare_iter::<ReceiptSchema, _>(&key_prefix)?;
        let mut iter = prepare_iter.ref_to_iter();

        let set = hashes.iter().collect::<HashSet<_>>();
        let mut found = HashMap::with_capacity(hashes.len());
        let mut count = hashes.len();

        while count > 0 {
            let (key, stx) = match iter.next() {
                None => break,
                Some(Ok(key_to_stx_bytes)) => key_to_stx_bytes,
                Some(Err(err)) => return Err(err),
            };

            if key.height() != block_height {
                break;
            }

            if set.contains(&stx.tx_hash) {
                found.insert(stx.tx_hash.clone(), stx);
                count -= 1;
            }
        }

        Ok(hashes
            .into_iter()
            .map(|h| found.remove(&h))
            .collect::<Vec<_>>())
    }

    async fn get_receipt_by_hash(
        &self,
        _ctx: Context,
        hash: Hash,
    ) -> ProtocolResult<Option<Receipt>> {
        if let Some(block_height) = get!(self, hash.clone(), HashHeightSchema)? {
            get!(self, CommonHashKey::new(block_height, hash), ReceiptSchema)
        } else {
            Ok(None)
        }
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn update_latest_proof(&self, ctx: Context, proof: Proof) -> ProtocolResult<()> {
        self.adapter
            .insert::<LatestProofSchema>(LATEST_PROOF_KEY.clone(), proof)
            .await?;
        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_latest_proof(&self, ctx: Context) -> ProtocolResult<Proof> {
        let proof = ensure_get!(self, LATEST_PROOF_KEY.clone(), LatestProofSchema);
        Ok(proof)
    }

    #[muta_apm::derive::tracing_span(kind = "storage")]
    async fn get_latest_block(&self, ctx: Context) -> ProtocolResult<Block> {
        let opt_block = { self.latest_block.read().await.clone() };

        if let Some(block) = opt_block {
            Ok(block)
        } else {
            let block = ensure_get!(self, LATEST_BLOCK_KEY.clone(), LatestBlockSchema);
            Ok(block)
        }
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
        let wal_info = ensure_get!(self, OVERLORD_WAL_KEY.clone(), OverlordWalSchema);
        Ok(wal_info)
    }
}

#[derive(Debug, Display, From)]
pub enum StorageError {
    #[display(fmt = "get none")]
    GetNone,

    #[display(fmt = "decode batch value")]
    BatchDecode,
}

impl Error for StorageError {}

impl From<StorageError> for ProtocolError {
    fn from(err: StorageError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
    }
}
