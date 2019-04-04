use std::collections::HashMap;
use std::sync::Arc;

use byteorder::{ByteOrder, NativeEndian};
use bytes::BytesMut;
use futures::future::{join_all, Future};

use core_runtime::{DataCategory, Database, FutRuntimeResult};
use core_serialization::{
    block::Block as PbBlock, receipt::Receipt as PbReceipt,
    transaction::SignedTransaction as PbSignedTransaction,
    transaction::TransactionPosition as PbTransactionPosition, AsyncCodec,
};
use core_types::{Block, Hash, Receipt, SignedTransaction, TransactionPosition};

use crate::errors::StorageError;

const LATEST_BLOCK: &[u8] = b"latest-block";

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

    /// Get a transaction position by hash.
    fn get_transaction_position(
        &self,
        hash: &Hash,
    ) -> FutRuntimeResult<TransactionPosition, StorageError>;

    /// Get a batch of transactions by hashes.
    fn get_transaction_positions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<TransactionPosition>>, StorageError>;

    /// Insert a block.
    fn insert_block(&self, block: &Block) -> FutRuntimeResult<(), StorageError>;

    /// Insert a batch of transactions.
    fn insert_transactions(
        &self,
        signed_txs: &[SignedTransaction],
    ) -> FutRuntimeResult<(), StorageError>;

    /// Insert a batch of transaction positions.
    fn insert_transaction_positions(
        &self,
        positions: &HashMap<Hash, TransactionPosition>,
    ) -> FutRuntimeResult<(), StorageError>;

    /// Insert a batch of receipts.
    fn insert_receipts(&self, receipts: &[Receipt]) -> FutRuntimeResult<(), StorageError>;
}

pub struct BlockStorage<DB>
where
    DB: Database,
{
    db: Arc<DB>,
}

impl<DB> BlockStorage<DB>
where
    DB: Database,
{
    pub fn new(db: Arc<DB>) -> Self {
        BlockStorage { db }
    }
}

impl<DB: 'static> Storage for BlockStorage<DB>
where
    DB: Database,
{
    fn get_latest_block(&self) -> FutRuntimeResult<Block, StorageError> {
        let fut = self
            .db
            .get(DataCategory::Block, LATEST_BLOCK)
            .map_err(StorageError::Database)
            .and_then(|data| AsyncCodec::decode::<PbBlock>(data).map_err(StorageError::Codec))
            .map(Block::from);

        Box::new(fut)
    }

    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Block, StorageError> {
        let key = transfrom_u64_to_array_u8(height);

        let fut = self
            .db
            .get(DataCategory::Block, &key)
            .map_err(StorageError::Database)
            .and_then(|data| AsyncCodec::decode::<PbBlock>(data).map_err(StorageError::Codec))
            .map(Block::from);;

        Box::new(fut)
    }

    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Block, StorageError> {
        let key = hash.clone();

        let db = Arc::clone(&self.db);

        let fut = self
            .db
            .get(DataCategory::Block, key.as_bytes())
            .map_err(StorageError::Database)
            .and_then(move |height_slice| {
                db.get(DataCategory::Block, &height_slice)
                    .map_err(StorageError::Database)
            })
            .and_then(|data| AsyncCodec::decode::<PbBlock>(data).map_err(StorageError::Codec))
            .map(Block::from);

        Box::new(fut)
    }

    fn get_transaction(&self, hash: &Hash) -> FutRuntimeResult<SignedTransaction, StorageError> {
        let key = hash.clone();

        let fut = self
            .db
            .get(DataCategory::Transaction, key.as_bytes())
            .map_err(StorageError::Database)
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
            keys.push(h.as_bytes().to_vec());
        }

        let fut = self
            .db
            .get_batch(DataCategory::Transaction, &keys)
            .map_err(StorageError::Database)
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
            .db
            .get(DataCategory::Receipt, key.as_bytes())
            .map_err(StorageError::Database)
            .and_then(|data| AsyncCodec::decode::<PbReceipt>(data).map_err(StorageError::Codec))
            .map(Receipt::from);

        Box::new(fut)
    }

    fn get_receipts(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<Receipt>>, StorageError> {
        let mut keys = Vec::with_capacity(hashes.len());
        for h in hashes {
            keys.push(h.as_bytes().to_vec());
        }

        let fut = self
            .db
            .get_batch(DataCategory::Receipt, &keys)
            .map_err(StorageError::Database)
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

    fn get_transaction_position(
        &self,
        hash: &Hash,
    ) -> FutRuntimeResult<TransactionPosition, StorageError> {
        let key = hash.clone();

        let fut = self
            .db
            .get(DataCategory::TransactionPosition, key.as_bytes())
            .map_err(StorageError::Database)
            .and_then(|data| {
                AsyncCodec::decode::<PbTransactionPosition>(data).map_err(StorageError::Codec)
            })
            .map(TransactionPosition::from);

        Box::new(fut)
    }

    fn get_transaction_positions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<TransactionPosition>>, StorageError> {
        let mut keys = vec![];
        for h in hashes {
            keys.push(h.as_bytes().to_vec());
        }

        let fut = self
            .db
            .get_batch(DataCategory::TransactionPosition, &keys)
            .map_err(StorageError::Database)
            .and_then(move |opt_txs_data| {
                join_all(opt_txs_data.into_iter().map(|opt_data| {
                    if let Some(data) = opt_data {
                        Some(
                            AsyncCodec::decode::<PbTransactionPosition>(data.to_vec())
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
                            Some(TransactionPosition::from(tx))
                        } else {
                            None
                        }
                    })
                    .collect()
            });

        Box::new(fut)
    }

    fn insert_block(&self, block: &Block) -> FutRuntimeResult<(), StorageError> {
        let db = Arc::clone(&self.db);

        let pb_block: PbBlock = block.clone().into();
        let mut encoded_buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_block));

        let height = block.header.height;
        let height_key = transfrom_u64_to_array_u8(block.header.height);
        let hash_key = block.header.hash();

        let fut = AsyncCodec::encode(&pb_block, &mut encoded_buf)
            .map_err(StorageError::Codec)
            .and_then(move |()| {
                join_all(vec![
                    db.insert(DataCategory::Block, &height_key, encoded_buf.as_ref())
                        .map_err(StorageError::Database),
                    db.insert(
                        DataCategory::Block,
                        hash_key.as_bytes(),
                        &transfrom_u64_to_array_u8(height),
                    )
                    .map_err(StorageError::Database),
                    db.insert(DataCategory::Block, LATEST_BLOCK, encoded_buf.as_ref())
                        .map_err(StorageError::Database),
                ])
            })
            .map(|_| ());

        Box::new(fut)
    }

    fn insert_transactions(
        &self,
        signed_txs: &[SignedTransaction],
    ) -> FutRuntimeResult<(), StorageError> {
        let db = Arc::clone(&self.db);
        let mut keys = Vec::with_capacity(signed_txs.len());

        let mut peding_fut = Vec::with_capacity(signed_txs.len());
        for tx in signed_txs {
            let pb_tx: PbSignedTransaction = tx.clone().into();
            let mut buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_tx));

            let fut = AsyncCodec::encode(&pb_tx, &mut buf)
                .map_err(StorageError::Codec)
                .map(move |_| buf.to_vec());

            keys.push(tx.hash.as_bytes().to_vec());
            peding_fut.push(fut);
        }

        let fut = join_all(peding_fut).and_then(move |buf_list| {
            db.insert_batch(DataCategory::Transaction, &keys, &buf_list)
                .map_err(StorageError::Database)
        });

        Box::new(fut)
    }

    fn insert_transaction_positions(
        &self,
        positions: &HashMap<Hash, TransactionPosition>,
    ) -> FutRuntimeResult<(), StorageError> {
        let db = Arc::clone(&self.db);
        let mut keys = Vec::with_capacity(positions.len());

        let mut peding_fut = Vec::with_capacity(positions.len());
        for (key, position) in positions.clone().into_iter() {
            let pb_tx: PbTransactionPosition = position.into();
            let mut buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_tx));

            let fut = AsyncCodec::encode(&pb_tx, &mut buf)
                .map_err(StorageError::Codec)
                .map(move |_| buf.to_vec());

            keys.push(key.as_bytes().to_vec());
            peding_fut.push(fut);
        }

        let fut = join_all(peding_fut).and_then(move |buf_list| {
            db.insert_batch(DataCategory::TransactionPosition, &keys, &buf_list)
                .map_err(StorageError::Database)
        });

        Box::new(fut)
    }

    fn insert_receipts(&self, receipts: &[Receipt]) -> FutRuntimeResult<(), StorageError> {
        let db = Arc::clone(&self.db);
        let mut keys = Vec::with_capacity(receipts.len());

        let mut peding_fut = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            let pb_receipt: PbReceipt = receipt.clone().into();
            let mut buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_receipt));

            let fut = AsyncCodec::encode(&pb_receipt, &mut buf)
                .map_err(StorageError::Codec)
                .map(move |_| buf.to_vec());

            keys.push(receipt.transaction_hash.as_bytes().to_vec());
            peding_fut.push(fut);
        }

        let fut = join_all(peding_fut).and_then(move |buf_list| {
            db.insert_batch(DataCategory::Receipt, &keys, &buf_list)
                .map_err(StorageError::Database)
        });

        Box::new(fut)
    }
}

impl<DB> Clone for BlockStorage<DB>
where
    DB: Database,
{
    fn clone(&self) -> Self {
        BlockStorage {
            db: Arc::clone(&self.db),
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
    use std::collections::HashMap;
    use std::sync::Arc;

    use futures::future::Future;

    use super::{BlockStorage, Storage};

    use components_database::memory::MemoryDB;
    use core_types::{
        Block, Hash, Receipt, SignedTransaction, TransactionPosition, UnverifiedTransaction,
    };

    #[test]
    fn test_get_latest_block_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        storage.insert_block(&mock_block(1000)).wait().unwrap();
        let block = storage.get_latest_block().wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_height_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        storage.insert_block(&mock_block(1000)).wait().unwrap();
        let block = storage.get_block_by_height(1000).wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_hash_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);

        let b = mock_block(1000);
        storage.insert_block(&b).wait().unwrap();

        let b = storage.get_block_by_hash(&b.header.hash()).wait().unwrap();
        assert_eq!(b.header.height, 1000)
    }

    #[test]
    fn test_get_transaction_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx = mock_transaction(Hash::digest(b"test111"));

        let hash = tx.hash.clone();
        storage.insert_transactions(&[tx]).wait().unwrap();
        let new_tx = storage.get_transaction(&hash).wait().unwrap();

        assert_eq!(new_tx.hash, hash)
    }

    #[test]
    fn test_get_transactions_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx1 = mock_transaction(Hash::digest(b"test111"));
        let tx2 = mock_transaction(Hash::digest(b"test222"));

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

        assert!(hashes.contains(&tx_hash1));
        assert!(hashes.contains(&tx_hash2));
    }

    #[test]
    fn test_transaction_position_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx_position = mock_transaction_position(Hash::default(), 0);

        let hash = Hash::digest(b"test");
        let mut positions = HashMap::new();
        positions.insert(hash.clone(), tx_position.clone());
        storage
            .insert_transaction_positions(&positions)
            .wait()
            .unwrap();
        let new_tx_position = storage.get_transaction_position(&hash).wait().unwrap();

        assert_eq!(new_tx_position, tx_position)
    }

    #[test]
    fn test_get_transaction_positions_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx_position1 = mock_transaction_position(Hash::default(), 0);
        let tx_position2 = mock_transaction_position(Hash::default(), 1);

        let hash1 = Hash::digest(b"test");
        let hash2 = Hash::digest(b"test2");

        let mut positions = HashMap::new();
        positions.insert(hash1.clone(), tx_position1.clone());
        positions.insert(hash2.clone(), tx_position2.clone());
        storage
            .insert_transaction_positions(&positions)
            .wait()
            .unwrap();
        let tx_positions = storage
            .get_transaction_positions(&[&hash1, &hash2])
            .wait()
            .unwrap();
        assert_eq!(tx_positions.len(), 2);

        assert!(tx_positions.contains(&Some(tx_position1)));
        assert!(tx_positions.contains(&Some(tx_position2)));
    }

    #[test]
    fn test_get_receipt_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let receipt = mock_receipt(Hash::digest(b"test111"));
        let tx_hash = receipt.transaction_hash.clone();

        storage.insert_receipts(&[receipt]).wait().unwrap();
        let receipt = storage.get_receipt(&tx_hash).wait().unwrap();
        assert_eq!(receipt.transaction_hash, tx_hash);
    }

    #[test]
    fn test_get_receipts_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let receipt1 = mock_receipt(Hash::digest(b"test111"));
        let receipt2 = mock_receipt(Hash::digest(b"test222"));

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

        assert!(hashes.contains(&tx_hash1));
        assert!(hashes.contains(&tx_hash2));
    }

    #[test]
    fn test_insert_block_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);

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
                .get_block_by_hash(&block.header.hash())
                .wait()
                .unwrap()
                .header
                .height,
            block.header.height
        );
    }

    fn mock_block(height: u64) -> Block {
        let mut b = Block::default();
        b.header.prevhash = Hash::digest(b"test");
        b.header.timestamp = 1234;
        b.header.height = height;
        b.tx_hashes = vec![Hash::digest(b"tx1"), Hash::digest(b"tx2")];
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

    fn mock_transaction_position(block_hash: Hash, position: u32) -> TransactionPosition {
        TransactionPosition {
            block_hash,
            position,
        }
    }
}
