use std::sync::Arc;

use byteorder::{ByteOrder, NativeEndian};
use bytes::BytesMut;
use futures::future::{join_all, Future};

use core_runtime::{DataCategory, DatabaseFactory, DatabaseInstance, FutRuntimeResult};
use core_serialization::{
    block::Block as PbBlock, receipt::Receipt as PbReceipt,
    transaction::SignedTransaction as PbSignedTransaction, AsyncCodec,
};
use core_types::{Block, Hash, Receipt, SignedTransaction};

use crate::errors::StorageError;

const LATEST_BLOCK: &[u8] = b"latest-block";

/// "storage" is responsible for the storage and retrieval of blockchain data.
/// Block, transaction, and receipt can be obtained from here,
/// but data related to "world status" is not available.
pub trait Storage: Send + Sync + Clone {
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

pub struct BlockStorage<F>
where
    F: DatabaseFactory,
{
    factory: Arc<F>,
}

impl<F> BlockStorage<F>
where
    F: DatabaseFactory,
{
    pub fn new(factory: Arc<F>) -> Self {
        BlockStorage { factory }
    }
}

impl<F: 'static> Storage for BlockStorage<F>
where
    F: DatabaseFactory,
{
    fn get_latest_block(&self) -> FutRuntimeResult<Block, StorageError> {
        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(|db_instance| {
                db_instance
                    .get(DataCategory::Block, LATEST_BLOCK)
                    .map_err(StorageError::Database)
            })
            .and_then(|data| AsyncCodec::decode::<PbBlock>(data).map_err(StorageError::Codec))
            .map(Block::from);

        Box::new(fut)
    }

    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Block, StorageError> {
        let key = transfrom_u64_to_array_u8(height);

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| {
                db_instance
                    .get(DataCategory::Block, &key)
                    .map_err(StorageError::Database)
            })
            .and_then(|data| AsyncCodec::decode::<PbBlock>(data).map_err(StorageError::Codec))
            .map(Block::from);;

        Box::new(fut)
    }

    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Block, StorageError> {
        let key = hash.clone();

        let instance_fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database);

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| {
                db_instance
                    .get(DataCategory::Block, key.as_ref())
                    .map_err(StorageError::Database)
            })
            .join(instance_fut)
            .and_then(move |(height_slice, db_instance)| {
                db_instance
                    .get(DataCategory::Block, &height_slice)
                    .map_err(StorageError::Database)
            })
            .and_then(|data| AsyncCodec::decode::<PbBlock>(data).map_err(StorageError::Codec))
            .map(Block::from);

        Box::new(fut)
    }

    fn get_transaction(&self, hash: &Hash) -> FutRuntimeResult<SignedTransaction, StorageError> {
        let key = hash.clone();

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| {
                db_instance
                    .get(DataCategory::Transaction, key.as_ref())
                    .map_err(StorageError::Database)
            })
            .and_then(|data| {
                AsyncCodec::decode::<PbSignedTransaction>(data).map_err(StorageError::Codec)
            })
            .map(SignedTransaction::from);

        Box::new(fut)
    }

    fn get_transactions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<SignedTransaction>>, StorageError> {
        let mut keys = vec![];
        for h in hashes {
            keys.push(h.as_ref().to_vec());
        }

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| {
                db_instance
                    .get_batch(DataCategory::Transaction, &keys)
                    .map_err(StorageError::Database)
            })
            .and_then(move |opt_txs_data| {
                join_all(opt_txs_data.into_iter().map(|opt_data| {
                    if let Some(data) = opt_data {
                        Some(
                            AsyncCodec::decode::<PbSignedTransaction>(data.to_vec())
                                .map_err(StorageError::Codec),
                        )
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
        let key = hash.clone();

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| {
                db_instance
                    .get(DataCategory::Receipt, key.as_ref())
                    .map_err(StorageError::Database)
            })
            .and_then(|data| AsyncCodec::decode::<PbReceipt>(data).map_err(StorageError::Codec))
            .map(Receipt::from);

        Box::new(fut)
    }

    fn get_receipts(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<Receipt>>, StorageError> {
        let mut keys = vec![];
        for h in hashes {
            keys.push(h.as_ref().to_vec());
        }

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| {
                db_instance
                    .get_batch(DataCategory::Receipt, &keys)
                    .map_err(StorageError::Database)
            })
            .and_then(|opt_receipts_data| {
                join_all(opt_receipts_data.into_iter().map(|opt_data| {
                    if let Some(data) = opt_data {
                        Some(
                            AsyncCodec::decode::<PbReceipt>(data.to_vec())
                                .map_err(StorageError::Codec),
                        )
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
        let instance_fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database);

        let pb_block: PbBlock = block.clone().into();
        let mut encoded_buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_block));

        let height = block.header.height;
        let height_key = transfrom_u64_to_array_u8(block.header.height);
        let hash_key = block.hash();

        let fut = AsyncCodec::encode(&pb_block, &mut encoded_buf)
            .map_err(StorageError::Codec)
            .and_then(|()| instance_fut)
            .and_then(move |mut db_instance| {
                join_all(vec![
                    db_instance
                        .insert(DataCategory::Block, &height_key, encoded_buf.as_ref())
                        .map_err(StorageError::Database),
                    db_instance
                        .insert(
                            DataCategory::Block,
                            hash_key.as_ref(),
                            &transfrom_u64_to_array_u8(height),
                        )
                        .map_err(StorageError::Database),
                    db_instance
                        .insert(DataCategory::Block, LATEST_BLOCK, encoded_buf.as_ref())
                        .map_err(StorageError::Database),
                ])
            })
            .map(|_| ());

        Box::new(fut)
    }

    fn insert_transactions(
        &mut self,
        signed_txs: &[SignedTransaction],
    ) -> FutRuntimeResult<(), StorageError> {
        let instance_fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database);

        let mut keys = vec![];

        let mut peding_fut = vec![];
        for tx in signed_txs {
            let pb_tx: PbSignedTransaction = tx.clone().into();
            let mut buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_tx));

            let fut = AsyncCodec::encode(&pb_tx, &mut buf)
                .map_err(StorageError::Codec)
                .map(move |_| buf.to_vec());

            keys.push(tx.hash.as_ref().to_vec());
            peding_fut.push(fut);
        }

        let fut =
            join_all(peding_fut)
                .join(instance_fut)
                .and_then(move |(buf_list, mut db_instance)| {
                    db_instance
                        .insert_batch(DataCategory::Transaction, &keys, &buf_list)
                        .map_err(StorageError::Database)
                });

        Box::new(fut)
    }

    fn insert_receipts(&mut self, receipts: &[Receipt]) -> FutRuntimeResult<(), StorageError> {
        let instance_fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database);

        let mut keys = vec![];

        let mut peding_fut = vec![];
        for receipt in receipts {
            let pb_receipt: PbReceipt = receipt.clone().into();
            let mut buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_receipt));

            let fut = AsyncCodec::encode(&pb_receipt, &mut buf)
                .map_err(StorageError::Codec)
                .map(move |_| buf.to_vec());

            keys.push(receipt.transaction_hash.as_ref().to_vec());
            peding_fut.push(fut);
        }

        let fut =
            join_all(peding_fut)
                .join(instance_fut)
                .and_then(move |(buf_list, mut db_instance)| {
                    db_instance
                        .insert_batch(DataCategory::Receipt, &keys, &buf_list)
                        .map_err(StorageError::Database)
                });

        Box::new(fut)
    }
}

impl<F> Clone for BlockStorage<F>
where
    F: DatabaseFactory,
{
    fn clone(&self) -> Self {
        BlockStorage {
            factory: Arc::clone(&self.factory),
        }
    }
}

fn transfrom_u64_to_array_u8(n: u64) -> Vec<u8> {
    let mut u64_slice = [0u8; 8];
    NativeEndian::write_u64(&mut u64_slice, n);
    u64_slice.to_vec()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::future::Future;

    use super::{BlockStorage, Storage};

    use components_database::memory::Factory;
    use core_types::{Block, Hash, Receipt, SignedTransaction, UnverifiedTransaction};

    #[test]
    fn test_get_latest_block_should_return_ok() {
        let factory = Arc::new(Factory::new());
        let mut storage = BlockStorage::new(factory);
        storage.insert_block(&mock_block(1000)).wait().unwrap();
        let block = storage.get_latest_block().wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_height_should_return_ok() {
        let factory = Arc::new(Factory::new());
        let mut storage = BlockStorage::new(factory);
        storage.insert_block(&mock_block(1000)).wait().unwrap();
        let block = storage.get_block_by_height(1000).wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_hash_should_return_ok() {
        let factory = Arc::new(Factory::new());
        let mut storage = BlockStorage::new(factory);

        let b = mock_block(1000);
        storage.insert_block(&b).wait().unwrap();

        let b = storage.get_block_by_hash(&b.hash()).wait().unwrap();
        assert_eq!(b.header.height, 1000)
    }

    #[test]
    fn test_get_transaction_should_return_ok() {
        let factory = Arc::new(Factory::new());
        let mut storage = BlockStorage::new(factory);
        let tx = mock_transaction(Hash::from_raw(b"test111"));

        let hash = tx.hash.clone();
        storage.insert_transactions(&[tx]).wait().unwrap();
        let new_tx = storage.get_transaction(&hash).wait().unwrap();

        assert_eq!(new_tx.hash, hash)
    }

    #[test]
    fn test_get_transactions_should_return_ok() {
        let factory = Arc::new(Factory::new());
        let mut storage = BlockStorage::new(factory);
        let tx1 = mock_transaction(Hash::from_raw(b"test111"));
        let tx2 = mock_transaction(Hash::from_raw(b"test222"));

        let tx_hash1 = tx1.hash.clone();
        let tx_hash2 = tx2.hash.clone();
        storage.insert_transactions(&[tx1, tx2]).wait().unwrap();
        let transactions = storage
            .get_transactions(&[&tx_hash1, &tx_hash2])
            .wait()
            .unwrap();
        assert_eq!(transactions.len(), 2);

        let hashes: Vec<Hash> = transactions
            .into_iter()
            .map(|opt_tx| opt_tx.unwrap().hash)
            .collect();

        assert_eq!(hashes.contains(&tx_hash1), true);
        assert_eq!(hashes.contains(&tx_hash2), true);
    }

    #[test]
    fn test_get_receipt_should_return_ok() {
        let factory = Arc::new(Factory::new());
        let mut storage = BlockStorage::new(factory);
        let receipt = mock_receipt(Hash::from_raw(b"test111"));
        let tx_hash = receipt.transaction_hash.clone();

        storage.insert_receipts(&[receipt]).wait().unwrap();
        let receipt = storage.get_receipt(&tx_hash).wait().unwrap();
        assert_eq!(receipt.transaction_hash, tx_hash);
    }

    #[test]
    fn test_get_receipts_should_return_ok() {
        let factory = Arc::new(Factory::new());
        let mut storage = BlockStorage::new(factory);
        let receipt1 = mock_receipt(Hash::from_raw(b"test111"));
        let receipt2 = mock_receipt(Hash::from_raw(b"test222"));

        let tx_hash1 = receipt1.transaction_hash.clone();
        let tx_hash2 = receipt2.transaction_hash.clone();
        storage
            .insert_receipts(&[receipt1, receipt2])
            .wait()
            .unwrap();
        let transactions = storage
            .get_receipts(&[&tx_hash1, &tx_hash2])
            .wait()
            .unwrap();
        assert_eq!(transactions.len(), 2);

        let hashes: Vec<Hash> = transactions
            .into_iter()
            .map(|opt_receipt| opt_receipt.unwrap().transaction_hash)
            .collect();

        assert_eq!(hashes.contains(&tx_hash1), true);
        assert_eq!(hashes.contains(&tx_hash2), true);
    }

    #[test]
    fn test_insert_block_should_return_ok() {
        let factory = Arc::new(Factory::new());
        let mut storage = BlockStorage::new(factory);

        let block = mock_block(1000);
        storage.insert_block(&block).wait().unwrap();
        assert_eq!(
            storage.get_latest_block().wait().unwrap().header.height,
            block.header.height
        );
        assert_eq!(
            storage
                .get_block_by_height(block.header.height)
                .wait()
                .unwrap()
                .header
                .height,
            block.header.height
        );

        assert_eq!(
            storage
                .get_block_by_hash(&block.hash())
                .wait()
                .unwrap()
                .header
                .height,
            block.header.height
        );
    }

    fn mock_block(height: u64) -> Block {
        let mut b = Block::default();
        b.header.prevhash = Hash::from_raw(b"test");
        b.header.timestamp = 1234;
        b.header.height = height;
        b.tx_hashes = vec![Hash::from_raw(b"tx1"), Hash::from_raw(b"tx2")];
        b
    }

    fn mock_transaction(tx_hash: Hash) -> SignedTransaction {
        let mut signed_tx = SignedTransaction::default();
        signed_tx.hash = tx_hash;
        signed_tx.untx = UnverifiedTransaction::default();
        signed_tx
    }

    fn mock_receipt(tx_hash: Hash) -> Receipt {
        let mut receipt = Receipt::default();
        receipt.transaction_hash = tx_hash;
        receipt
    }
}
