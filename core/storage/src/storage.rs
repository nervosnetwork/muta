use std::collections::HashMap;
use std::sync::Arc;

use byteorder::{ByteOrder, NativeEndian};
use bytes::BytesMut;
use futures::future::{err, join_all, ok, Future};

use core_runtime::{DataCategory, Database, DatabaseError};
use core_serialization::{
    AsyncCodec, Block as SerBlock, Receipt as SerReceipt,
    SignedTransaction as SerSignedTransaction, TransactionPosition as SerTransactionPosition,
};
use core_types::{Block, Hash, Receipt, SignedTransaction, TransactionPosition};

use crate::errors::{StorageError, StorageResult};

const LATEST_BLOCK: &[u8] = b"latest-block";

/// "storage" is responsible for the storage and retrieval of blockchain data.
/// Block, transaction, and receipt can be obtained from here,
/// but data related to "world status" is not available.
pub trait Storage: Send + Sync {
    /// Get the latest block.
    fn get_latest_block(&self) -> StorageResult<Block>;

    /// Get a block by height.
    fn get_block_by_height(&self, height: u64) -> StorageResult<Option<Block>>;

    /// Get a block by hash.
    /// The hash is actually an index,
    /// and the corresponding height is obtained by hash and then querying the corresponding block.
    fn get_block_by_hash(&self, hash: &Hash) -> StorageResult<Option<Block>>;

    /// Get a signed transaction by hash.
    fn get_transaction(&self, hash: &Hash) -> StorageResult<Option<SignedTransaction>>;

    /// Get a batch of transactions by hashes.
    fn get_transactions(&self, hashes: &[&Hash]) -> StorageResult<Vec<Option<SignedTransaction>>>;

    /// Get a receipt by hash.
    fn get_receipt(&self, tx_hash: &Hash) -> StorageResult<Option<Receipt>>;

    /// Get a batch of receipts by hashes
    fn get_receipts(&self, tx_hashes: &[&Hash]) -> StorageResult<Vec<Option<Receipt>>>;

    /// Get a transaction position by hash.
    fn get_transaction_position(&self, hash: &Hash) -> StorageResult<Option<TransactionPosition>>;

    /// Get a batch of transactions by hashes.
    fn get_transaction_positions(
        &self,
        hashes: &[&Hash],
    ) -> StorageResult<Vec<Option<TransactionPosition>>>;

    /// Insert a block.
    fn insert_block(&self, block: Block) -> StorageResult<()>;

    /// Insert a batch of transactions.
    fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> StorageResult<()>;

    /// Insert a batch of transaction positions.
    fn insert_transaction_positions(
        &self,
        positions: HashMap<Hash, TransactionPosition>,
    ) -> StorageResult<()>;

    /// Insert a batch of receipts.
    fn insert_receipts(&self, receipts: Vec<Receipt>) -> StorageResult<()>;
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
    fn get_latest_block(&self) -> StorageResult<Block> {
        let decode = |d| -> Box<dyn Future<Item = _, Error = _> + Send> {
            match d {
                Some(data) => {
                    Box::new(AsyncCodec::decode::<SerBlock>(data).map_err(StorageError::Codec))
                }
                None => Box::new(err(StorageError::Database(DatabaseError::NotFound))),
            }
        };

        let fut = self
            .db
            .get(DataCategory::Block, LATEST_BLOCK)
            .map_err(StorageError::Database)
            .and_then(decode)
            .map(SerBlock::into);

        Box::new(fut)
    }

    fn get_block_by_height(&self, height: u64) -> StorageResult<Option<Block>> {
        let key = transfrom_u64_to_array_u8(height);

        let fut = self
            .db
            .get(DataCategory::Block, &key)
            .map_err(StorageError::Database)
            .and_then(|data| {
                data.map(|v| AsyncCodec::decode::<SerBlock>(v).map_err(StorageError::Codec))
            })
            .map(|b| b.map(SerBlock::into));;

        Box::new(fut)
    }

    fn get_block_by_hash(&self, hash: &Hash) -> StorageResult<Option<Block>> {
        let key = hash.clone();

        let db = Arc::clone(&self.db);

        let get_block =
            move |height_slice: Option<Vec<u8>>| -> Box<dyn Future<Item = _, Error = _> + Send> {
                match height_slice {
                    Some(h) => Box::new(
                        db.get(DataCategory::Block, &h)
                            .map_err(StorageError::Database),
                    ), // StorageResult<Option<Vec<u8>>>
                    None => Box::new(ok(None)),
                }
            };

        let fut = self
            .db
            .get(DataCategory::Block, key.as_bytes())
            .map_err(StorageError::Database)
            .and_then(get_block)
            .and_then(|data| {
                data.map(|value| AsyncCodec::decode::<SerBlock>(value).map_err(StorageError::Codec))
            })
            .map(|b| b.map(SerBlock::into));

        Box::new(fut)
    }

    fn get_transaction(&self, hash: &Hash) -> StorageResult<Option<SignedTransaction>> {
        let key = hash.clone();

        let fut = self
            .db
            .get(DataCategory::Transaction, key.as_bytes())
            .map_err(StorageError::Database)
            .and_then(|data| {
                data.map(|v| {
                    AsyncCodec::decode::<SerSignedTransaction>(v).map_err(StorageError::Codec)
                })
            })
            .map(|b| b.map(SerSignedTransaction::into));

        Box::new(fut)
    }

    fn get_transactions(&self, hashes: &[&Hash]) -> StorageResult<Vec<Option<SignedTransaction>>> {
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
                    opt_data.map(|v| {
                        AsyncCodec::decode::<SerSignedTransaction>(v.to_vec())
                            .map_err(StorageError::Codec)
                    })
                }))
            })
            .map(|opt_txs| {
                opt_txs
                    .into_iter()
                    .map(|opt_tx| {
                        if let Some(tx) = opt_tx {
                            Some(tx.into())
                        } else {
                            None
                        }
                    })
                    .collect()
            });

        Box::new(fut)
    }

    fn get_receipt(&self, hash: &Hash) -> StorageResult<Option<Receipt>> {
        let key = hash.clone();

        let fut = self
            .db
            .get(DataCategory::Receipt, key.as_bytes())
            .map_err(StorageError::Database)
            .and_then(|data| {
                data.map(|v| AsyncCodec::decode::<SerReceipt>(v).map_err(StorageError::Codec))
            })
            .map(|b| b.map(SerReceipt::into));

        Box::new(fut)
    }

    fn get_receipts(&self, hashes: &[&Hash]) -> StorageResult<Vec<Option<Receipt>>> {
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
                    opt_data.map(|v| {
                        AsyncCodec::decode::<SerReceipt>(v.to_vec()).map_err(StorageError::Codec)
                    })
                }))
            })
            .map(|opt_txs| {
                opt_txs
                    .into_iter()
                    .map(|opt_tx| {
                        if let Some(tx) = opt_tx {
                            Some(tx.into())
                        } else {
                            None
                        }
                    })
                    .collect()
            });
        Box::new(fut)
    }

    fn get_transaction_position(&self, hash: &Hash) -> StorageResult<Option<TransactionPosition>> {
        let key = hash.clone();

        let fut = self
            .db
            .get(DataCategory::TransactionPosition, key.as_bytes())
            .map_err(StorageError::Database)
            .and_then(|data| {
                data.map(|v| {
                    AsyncCodec::decode::<SerTransactionPosition>(v).map_err(StorageError::Codec)
                })
            })
            .map(|b| b.map(SerTransactionPosition::into));

        Box::new(fut)
    }

    // TODO: refactor
    fn get_transaction_positions(
        &self,
        hashes: &[&Hash],
    ) -> StorageResult<Vec<Option<TransactionPosition>>> {
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
                            AsyncCodec::decode::<SerTransactionPosition>(data.to_vec())
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
                            Some(tx.into())
                        } else {
                            None
                        }
                    })
                    .collect()
            });

        Box::new(fut)
    }

    fn insert_block(&self, block: Block) -> StorageResult<()> {
        let db = Arc::clone(&self.db);

        let height = block.header.height;
        let height_key = transfrom_u64_to_array_u8(block.header.height);
        let hash_key = block.header.hash();

        let pb_block: SerBlock = block.into();
        let mut encoded_buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_block));

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

    fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> StorageResult<()> {
        let db = Arc::clone(&self.db);
        let mut keys = Vec::with_capacity(signed_txs.len());

        let mut peding_fut = Vec::with_capacity(signed_txs.len());
        for tx in signed_txs {
            let hash = tx.hash.clone();
            let pb_tx: SerSignedTransaction = tx.into();
            let mut buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_tx));

            let fut = AsyncCodec::encode(&pb_tx, &mut buf)
                .map_err(StorageError::Codec)
                .map(move |_| buf.to_vec());

            keys.push(hash.as_bytes().to_vec());
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
        positions: HashMap<Hash, TransactionPosition>,
    ) -> StorageResult<()> {
        let db = Arc::clone(&self.db);
        let mut keys = Vec::with_capacity(positions.len());

        let mut peding_fut = Vec::with_capacity(positions.len());
        for (key, position) in positions.into_iter() {
            let pb_tx: SerTransactionPosition = position.into();
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

    fn insert_receipts(&self, receipts: Vec<Receipt>) -> StorageResult<()> {
        let db = Arc::clone(&self.db);
        let mut keys = Vec::with_capacity(receipts.len());

        let mut peding_fut = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            let hash = receipt.transaction_hash.clone();
            let pb_receipt: SerReceipt = receipt.into();
            let mut buf = BytesMut::with_capacity(AsyncCodec::encoded_len(&pb_receipt));

            let fut = AsyncCodec::encode(&pb_receipt, &mut buf)
                .map_err(StorageError::Codec)
                .map(move |_| buf.to_vec());

            keys.push(hash.as_bytes().to_vec());
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
        storage.insert_block(mock_block(1000)).wait().unwrap();
        let block = storage.get_latest_block().wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_height_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        storage.insert_block(mock_block(1000)).wait().unwrap();
        let block = storage.get_block_by_height(1000).wait().unwrap();

        assert_eq!(block.unwrap().header.height, 1000)
    }

    #[test]
    fn test_get_block_by_hash_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);

        let b = mock_block(1000);
        let hash = b.header.hash().clone();
        storage.insert_block(b).wait().unwrap();

        let b = storage.get_block_by_hash(&hash).wait().unwrap();
        assert_eq!(b.unwrap().header.height, 1000)
    }

    #[test]
    fn test_get_transaction_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx = mock_transaction(Hash::digest(b"test111"));

        let hash = tx.hash.clone();
        storage.insert_transactions(vec![tx]).wait().unwrap();
        let new_tx = storage.get_transaction(&hash).wait().unwrap();

        assert_eq!(new_tx.unwrap().hash, hash)
    }

    #[test]
    fn test_get_transactions_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx1 = mock_transaction(Hash::digest(b"test111"));
        let tx2 = mock_transaction(Hash::digest(b"test222"));

        let tx_hash1 = tx1.hash.clone();
        let tx_hash2 = tx2.hash.clone();
        storage.insert_transactions(vec![tx1, tx2]).wait().unwrap();
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
            .insert_transaction_positions(positions)
            .wait()
            .unwrap();
        let new_tx_position = storage.get_transaction_position(&hash).wait().unwrap();

        assert_eq!(new_tx_position, Some(tx_position));
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
            .insert_transaction_positions(positions)
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

        storage.insert_receipts(vec![receipt]).wait().unwrap();
        let receipt = storage.get_receipt(&tx_hash).wait().unwrap();
        assert_eq!(receipt.unwrap().transaction_hash, tx_hash);
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
            .insert_receipts(vec![receipt1, receipt2])
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
        let height = block.header.height;
        let hash = block.header.hash().clone();
        storage.insert_block(block).wait().unwrap();
        assert_eq!(
            storage.get_latest_block().wait().unwrap().header.height,
            height
        );
        assert_eq!(
            storage
                .get_block_by_height(height)
                .wait()
                .unwrap()
                .unwrap()
                .header
                .height,
            height
        );

        assert_eq!(
            storage
                .get_block_by_hash(&hash)
                .wait()
                .unwrap()
                .unwrap()
                .header
                .height,
            height
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
