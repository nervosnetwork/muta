use std::sync::Arc;

use byteorder::{ByteOrder, NativeEndian};
use bytes::{BytesMut, IntoBuf};
use futures::future::{join_all, ok, result, Future, JoinAll};
use futures_locks::RwLock;
use prost::Message;

use core_runtime::{Database, FutRuntimeResult};
use core_serialization::{
    block::Block as PbBlock, receipt::Receipt as PbReceipt,
    transaction::SignedTransaction as PbSignedTransaction,
};
use core_types::{Block, Hash, Receipt, SignedTransaction};

use crate::errors::StorageError;

const PREFIX_LATEST_BLOCK: &[u8] = b"latest-block";
const PREFIX_BLOCK_HEIGHT_BY_HASH: &[u8] = b"block-hash-";
const PREFIX_BLOCK_HEIGHT: &[u8] = b"block-height-";
const PREFIX_TRANSACTION: &[u8] = b"transaction-";
const PREFIX_RECEIPT: &[u8] = b"receipt-";

/// "storage" is responsible for the storage and retrieval of blockchain data.
/// Block, transaction, and receipt can be obtained from here,
/// but data related to "world status" is not available.
pub trait Storage: Send + Sync {
    /// Get the latest block.
    fn get_latest_block(&self) -> FutRuntimeResult<Block, StorageError>;

    /// Get a block by height.
    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Block, StorageError>;

    /// Get a block by hash.
    /// The hash is actually an index,
    /// and the corresponding height is obtained by hash and then querying the corresponding block.
    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Block, StorageError>;

    /// Get a signed transaction by hash.
    fn get_transaction(&self, hash: &Hash) -> FutRuntimeResult<SignedTransaction, StorageError>;

    /// Get a batch of transactions by hashes.
    fn get_transactions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<SignedTransaction>>, StorageError>;

    /// Get a receipt by hash.
    fn get_receipt(&self, tx_hash: &Hash) -> FutRuntimeResult<Receipt, StorageError>;

    /// Get a batch of receipts by hashes
    fn get_receipts(
        &self,
        tx_hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<Receipt>>, StorageError>;

    /// Insert a block.
    fn insert_block(&mut self, block: &Block) -> FutRuntimeResult<(), StorageError>;

    /// Insert a batch of transactions.
    fn insert_transactions(
        &mut self,
        signed_txs: &[SignedTransaction],
    ) -> FutRuntimeResult<(), StorageError>;

    /// Insert a batch of receipts.
    fn insert_receipts(&mut self, receipts: &[Receipt]) -> FutRuntimeResult<(), StorageError>;
}

pub struct BlockStorage<DB>
where
    DB: Database,
{
    db: Arc<RwLock<DB>>,
}

impl<DB> BlockStorage<DB>
where
    DB: Database,
{
    pub fn new(db: Arc<RwLock<DB>>) -> Self {
        BlockStorage { db }
    }

    fn cloned(&self) -> Self {
        BlockStorage {
            db: Arc::clone(&self.db),
        }
    }
}

impl<DB: 'static> Storage for BlockStorage<DB>
where
    DB: Database,
{
    fn get_latest_block(&self) -> FutRuntimeResult<Block, StorageError> {
        let storage = self.cloned();

        let fut = storage
            .db
            .read()
            .map_err(|()| StorageError::Internal)
            .and_then(|db| db.get(PREFIX_LATEST_BLOCK).map_err(StorageError::Database))
            .and_then(AsyncCodec::decode::<PbBlock>)
            .map(Block::from);

        Box::new(fut)
    }

    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Block, StorageError> {
        let storage = self.cloned();
        let key = gen_key_with_u64(PREFIX_BLOCK_HEIGHT, height);

        let fut = storage
            .db
            .read()
            .map_err(|()| StorageError::Internal)
            .and_then(move |db| db.get(&key).map_err(StorageError::Database))
            .and_then(AsyncCodec::decode::<PbBlock>)
            .map(Block::from);

        Box::new(fut)
    }

    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Block, StorageError> {
        let storage = self.cloned();
        let key = gen_key_with_slice(PREFIX_BLOCK_HEIGHT_BY_HASH, hash.as_ref());

        let fut = storage
            .db
            .read()
            .map_err(|()| StorageError::Internal)
            .and_then(move |db| db.get(&key).map_err(StorageError::Database))
            .map(|height_slice| transfrom_array_u8_to_u64(&height_slice))
            .and_then(move |height| storage.cloned().get_block_by_height(height));

        Box::new(fut)
    }

    fn get_transaction(&self, hash: &Hash) -> FutRuntimeResult<SignedTransaction, StorageError> {
        let storage = self.cloned();
        let key = gen_key_with_slice(PREFIX_TRANSACTION, hash.as_ref());

        let fut = storage
            .db
            .read()
            .map_err(|()| StorageError::Internal)
            .and_then(move |db| db.get(&key).map_err(StorageError::Database))
            .and_then(AsyncCodec::decode::<PbSignedTransaction>)
            .map(SignedTransaction::from);

        Box::new(fut)
    }

    fn get_transactions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<SignedTransaction>>, StorageError> {
        let storage = self.cloned();
        let mut keys = vec![];
        for h in hashes {
            keys.push([PREFIX_TRANSACTION, h.as_ref()].concat());
        }

        let fut = storage
            .db
            .read()
            .map_err(|()| StorageError::Internal)
            .and_then(move |db| db.get_batch(&keys).map_err(StorageError::Database))
            .and_then(move |opt_txs_data| {
                join_all(opt_txs_data.into_iter().map(|opt_data| {
                    if let Some(data) = opt_data {
                        Some(AsyncCodec::decode::<PbSignedTransaction>(data.to_vec()))
                    } else {
                        None
                    }
                }))
            })
            .map(|opt_txs| {
                opt_txs
                    .into_iter()
                    .map(|opt_tx| {
                        if let Some(tx) = opt_tx {
                            Some(SignedTransaction::from(tx))
                        } else {
                            None
                        }
                    })
                    .collect()
            });
        Box::new(fut)
    }

    fn get_receipt(&self, hash: &Hash) -> FutRuntimeResult<Receipt, StorageError> {
        let storage = self.cloned();
        let key = gen_key_with_slice(PREFIX_RECEIPT, hash.as_ref());

        let fut = storage
            .db
            .read()
            .map_err(|()| StorageError::Internal)
            .and_then(move |db| db.get(&key).map_err(StorageError::Database))
            .and_then(AsyncCodec::decode::<PbReceipt>)
            .map(Receipt::from);

        Box::new(fut)
    }

    fn get_receipts(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<Receipt>>, StorageError> {
        let storage = self.cloned();
        let mut keys = vec![];
        for h in hashes {
            keys.push([PREFIX_RECEIPT, h.as_ref()].concat());
        }

        let fut = storage
            .db
            .read()
            .map_err(|()| StorageError::Internal)
            .and_then(move |db| db.get_batch(&keys).map_err(StorageError::Database))
            .and_then(|opt_receipts_data| {
                join_all(opt_receipts_data.into_iter().map(|opt_data| {
                    if let Some(data) = opt_data {
                        Some(AsyncCodec::decode::<PbReceipt>(data.to_vec()))
                    } else {
                        None
                    }
                }))
            })
            .map(|opt_txs| {
                opt_txs
                    .into_iter()
                    .map(|opt_tx| {
                        if let Some(tx) = opt_tx {
                            Some(Receipt::from(tx))
                        } else {
                            None
                        }
                    })
                    .collect()
            });
        Box::new(fut)
    }

    fn insert_block(&mut self, block: &Block) -> FutRuntimeResult<(), StorageError> {
        let storage = self.cloned();
        let pb_block: PbBlock = block.clone().into();
        let height_key = gen_key_with_u64(PREFIX_BLOCK_HEIGHT, block.header.height);
        let hash_key = gen_key_with_slice(PREFIX_BLOCK_HEIGHT_BY_HASH, block.hash().as_ref());

        let fut = AsyncCodec::encode(pb_block)
            .join3(
                storage.db.write().map_err(|()| StorageError::Internal),
                storage.db.write().map_err(|()| StorageError::Internal),
            )
            .map(move |(encoded_data, mut db1, mut db2)| {
                [
                    db1.insert(&height_key, &encoded_data),
                    db2.insert(&hash_key, &encoded_data),
                ]
            })
            .map(|_| ());

        Box::new(fut)
    }

    fn insert_transactions(
        &mut self,
        signed_txs: &[SignedTransaction],
    ) -> FutRuntimeResult<(), StorageError> {
        let storage = self.cloned();
        let mut keys = vec![];
        let mut values = vec![];

        let fut_encoding: JoinAll<Vec<FutRuntimeResult<Vec<u8>, StorageError>>> = join_all(
            signed_txs
                .iter()
                .map(|tx| {
                    keys.push(gen_key_with_slice(PREFIX_TRANSACTION, tx.hash.as_ref()));
                    AsyncCodec::encode::<PbSignedTransaction>(tx.clone().into())
                })
                .collect(),
        );

        let fut = fut_encoding
            .join(storage.db.write().map_err(|()| StorageError::Internal))
            .and_then(move |(txs_data, mut db)| {
                txs_data.iter().for_each(|data| values.push(data.to_vec()));
                db.insert_batch(&keys, &values)
                    .map_err(StorageError::Database)
            });

        Box::new(fut)
    }

    fn insert_receipts(&mut self, receipts: &[Receipt]) -> FutRuntimeResult<(), StorageError> {
        let storage = self.cloned();
        let mut keys = vec![];
        let mut values = vec![];

        let fut_encoding: JoinAll<Vec<FutRuntimeResult<Vec<u8>, StorageError>>> = join_all(
            receipts
                .iter()
                .map(|receipt| {
                    keys.push(gen_key_with_slice(
                        PREFIX_TRANSACTION,
                        receipt.transaction_hash.as_ref(),
                    ));
                    AsyncCodec::encode::<PbReceipt>(receipt.clone().into())
                })
                .collect(),
        );

        let fut = fut_encoding
            .join(storage.db.write().map_err(|()| StorageError::Internal))
            .and_then(move |(receipts_data, mut db)| {
                receipts_data
                    .iter()
                    .for_each(|data| values.push(data.to_vec()));
                db.insert_batch(&keys, &values)
                    .map_err(StorageError::Database)
            });

        Box::new(fut)
    }
}

fn gen_key_with_slice(prefix: &[u8], value: &[u8]) -> Vec<u8> {
    [prefix, value].concat()
}

fn gen_key_with_u64(prefix: &[u8], n: u64) -> Vec<u8> {
    gen_key_with_slice(prefix, &transfrom_u64_to_array_u8(n))
}

fn transfrom_array_u8_to_u64(d: &[u8]) -> u64 {
    NativeEndian::read_u64(d)
}

fn transfrom_u64_to_array_u8(n: u64) -> Vec<u8> {
    let mut u64_slice = [];
    NativeEndian::write_u64(&mut u64_slice, n);
    u64_slice.to_vec()
}

#[derive(Default)]
struct AsyncCodec;

impl AsyncCodec {
    pub fn decode<T: 'static + Message + Default>(
        data: Vec<u8>,
    ) -> FutRuntimeResult<T, StorageError> {
        Box::new(result(
            T::decode(data.into_buf()).map_err(StorageError::Decode),
        ))
    }

    pub fn encode<T: Message>(msg: T) -> FutRuntimeResult<Vec<u8>, StorageError> {
        let mut b = BytesMut::new();
        let fut = result(msg.encode(&mut b).map_err(StorageError::Encode))
            .from_err()
            .and_then(move |_| ok(b.to_vec()));
        Box::new(fut)
    }
}
