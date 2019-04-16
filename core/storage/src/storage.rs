use std::collections::HashMap;
use std::sync::Arc;

use byteorder::{ByteOrder, NativeEndian};
use futures::{
    compat::Future01CompatExt,
    stream::{self, TryStreamExt},
};
use old_futures::future::Future as OldFuture;
use tokio_async_await::compat::backward::Compat;

use core_runtime::{DataCategory, Database};
use core_serialization::{
    AsyncCodec, Block as SerBlock, Receipt as SerReceipt,
    SignedTransaction as SerSignedTransaction, TransactionPosition as SerTransactionPosition,
};
use core_types::{Block, Hash, Receipt, SignedTransaction, TransactionPosition};

use crate::errors::StorageError;

const LATEST_BLOCK: &[u8] = b"latest-block";

pub type StorageResult<T> = Box<OldFuture<Item = T, Error = StorageError> + Send>;

/// "storage" is responsible for the storage and retrieval of blockchain data.
/// Block, transaction, and receipt can be obtained from here,
/// but data related to "world status" is not available.
/// NOTE: Anything that might return "std::option::None" will return "std::option:: NoneError".
pub trait Storage: Send + Sync {
    /// Get the latest block.
    fn get_latest_block(&self) -> StorageResult<Block>;

    /// Get a block by height.
    fn get_block_by_height(&self, height: u64) -> StorageResult<Block>;

    /// Get a block by hash.
    /// The hash is actually an index,
    /// and the corresponding height is obtained by hash and then querying the corresponding block.
    fn get_block_by_hash(&self, hash: &Hash) -> StorageResult<Block>;

    /// Get a signed transaction by hash.
    fn get_transaction(&self, hash: &Hash) -> StorageResult<SignedTransaction>;

    /// Get a batch of transactions by hashes.
    fn get_transactions(&self, hashes: &[Hash]) -> StorageResult<Vec<SignedTransaction>>;

    /// Get a receipt by hash.
    fn get_receipt(&self, tx_hash: &Hash) -> StorageResult<Receipt>;

    /// Get a batch of receipts by hashes
    fn get_receipts(&self, tx_hashes: &[Hash]) -> StorageResult<Vec<Receipt>>;

    /// Get a transaction position by hash.
    fn get_transaction_position(&self, hash: &Hash) -> StorageResult<TransactionPosition>;

    /// Get a batch of transactions by hashes.
    fn get_transaction_positions(&self, hashes: &[Hash])
        -> StorageResult<Vec<TransactionPosition>>;

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
        let db = Arc::clone(&self.db);

        let fut = async move {
            let value = await!(db.get(DataCategory::Block, LATEST_BLOCK).compat())?;

            let block = await!(AsyncCodec::decode::<SerBlock>(value?))?.into();
            Ok(block)
        };

        Box::new(Compat::new(fut))
    }

    fn get_block_by_height(&self, height: u64) -> StorageResult<Block> {
        let db = Arc::clone(&self.db);
        let key = transfrom_u64_to_array_u8(height);

        let fut = async move {
            let value = await!(db.get(DataCategory::Block, &key).compat())?;

            let block = await!(AsyncCodec::decode::<SerBlock>(value?))?.into();
            Ok(block)
        };

        Box::new(Compat::new(fut))
    }

    fn get_block_by_hash(&self, hash: &Hash) -> StorageResult<Block> {
        let db = Arc::clone(&self.db);
        let key = hash.clone();

        let fut = async move {
            let height_slice = await!(db.get(DataCategory::Block, key.as_bytes()).compat())?;
            let value = await!(db.get(DataCategory::Block, &height_slice?).compat())?;

            let block = await!(AsyncCodec::decode::<SerBlock>(value?))?.into();
            Ok(block)
        };

        Box::new(Compat::new(fut))
    }

    fn get_transaction(&self, hash: &Hash) -> StorageResult<SignedTransaction> {
        let db = Arc::clone(&self.db);
        let key = hash.clone();

        let fut = async move {
            let value = await!(db.get(DataCategory::Transaction, key.as_bytes()).compat())?;

            let tx = await!(AsyncCodec::decode::<SerSignedTransaction>(value?))?.into();
            Ok(tx)
        };

        Box::new(Compat::new(fut))
    }

    fn get_transactions(&self, hashes: &[Hash]) -> StorageResult<Vec<SignedTransaction>> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = hashes.iter().map(|h| h.as_bytes().to_vec()).collect();

        let fut = async move {
            let values = await!(db.get_batch(DataCategory::Transaction, &keys).compat())?;
            let values = opts_to_flat(values);

            let txs: Vec<SignedTransaction> =
                await!(AsyncCodec::decode_batch::<SerSignedTransaction>(values))?
                    .into_iter()
                    .map(Into::into)
                    .collect();
            Ok(txs)
        };

        Box::new(Compat::new(fut))
    }

    fn get_receipt(&self, hash: &Hash) -> StorageResult<Receipt> {
        let db = Arc::clone(&self.db);
        let key = hash.clone();

        let fut = async move {
            let value = await!(db.get(DataCategory::Receipt, key.as_bytes()).compat())?;

            let receipt = await!(AsyncCodec::decode::<SerReceipt>(value?))?.into();
            Ok(receipt)
        };

        Box::new(Compat::new(fut))
    }

    fn get_receipts(&self, hashes: &[Hash]) -> StorageResult<Vec<Receipt>> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = hashes.iter().map(|h| h.as_bytes().to_vec()).collect();

        let fut = async move {
            let values = await!(db.get_batch(DataCategory::Receipt, &keys).compat())?;
            let values = opts_to_flat(values);

            let receipts: Vec<Receipt> = await!(AsyncCodec::decode_batch::<SerReceipt>(values))?
                .into_iter()
                .map(Into::into)
                .collect();
            Ok(receipts)
        };

        Box::new(Compat::new(fut))
    }

    fn get_transaction_position(&self, hash: &Hash) -> StorageResult<TransactionPosition> {
        let db = Arc::clone(&self.db);
        let key = hash.clone();

        let fut = async move {
            let value = await!(db
                .get(DataCategory::TransactionPosition, key.as_bytes())
                .compat())?;

            let tx_position = await!(AsyncCodec::decode::<SerTransactionPosition>(value?))?.into();
            Ok(tx_position)
        };

        Box::new(Compat::new(fut))
    }

    fn get_transaction_positions(
        &self,
        hashes: &[Hash],
    ) -> StorageResult<Vec<TransactionPosition>> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = hashes.iter().map(|h| h.as_bytes().to_vec()).collect();

        let fut = async move {
            let values = await!(db
                .get_batch(DataCategory::TransactionPosition, &keys)
                .compat())?;
            let values = opts_to_flat(values);

            let tx_positions: Vec<TransactionPosition> =
                await!(AsyncCodec::decode_batch::<SerTransactionPosition>(values))?
                    .into_iter()
                    .map(Into::into)
                    .collect();
            Ok(tx_positions)
        };

        Box::new(Compat::new(fut))
    }

    fn insert_block(&self, block: Block) -> StorageResult<()> {
        let db = Arc::clone(&self.db);

        let height = block.header.height;
        let height_key = transfrom_u64_to_array_u8(block.header.height);
        let hash_key = block.header.hash();

        let pb_block: SerBlock = block.into();

        let fut = async move {
            let encode_value = await!(AsyncCodec::encode(pb_block))?;

            let stream = stream::futures_unordered(vec![
                db.insert(DataCategory::Block, height_key, encode_value.clone())
                    .compat(),
                db.insert(
                    DataCategory::Block,
                    hash_key.as_bytes().to_vec(),
                    transfrom_u64_to_array_u8(height),
                )
                .compat(),
                db.insert(
                    DataCategory::Block,
                    LATEST_BLOCK.to_vec(),
                    encode_value.clone(),
                )
                .compat(),
            ]);

            await!(stream.try_collect())?;
            Ok(())
        };

        Box::new(Compat::new(fut))
    }

    fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> StorageResult<()> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = signed_txs
            .iter()
            .map(|tx| tx.hash.as_bytes().to_vec())
            .collect();

        let fut = async move {
            let pb_txs: Vec<SerSignedTransaction> =
                signed_txs.into_iter().map(Into::into).collect();
            let values = await!(AsyncCodec::encode_batch(pb_txs))?;

            await!(db
                .insert_batch(DataCategory::Transaction, keys, values)
                .compat())?;
            Ok(())
        };

        Box::new(Compat::new(fut))
    }

    fn insert_transaction_positions(
        &self,
        positions: HashMap<Hash, TransactionPosition>,
    ) -> StorageResult<()> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            let mut keys: Vec<Vec<u8>> = Vec::with_capacity(positions.len());
            let mut ser_positions: Vec<SerTransactionPosition> =
                Vec::with_capacity(positions.len());

            for (key, position) in positions.into_iter() {
                keys.push(key.as_bytes().to_vec());
                ser_positions.push(position.into());
            }

            let values = await!(AsyncCodec::encode_batch(ser_positions))?;

            await!(db
                .insert_batch(DataCategory::TransactionPosition, keys, values)
                .compat())?;
            Ok(())
        };

        Box::new(Compat::new(fut))
    }

    fn insert_receipts(&self, receipts: Vec<Receipt>) -> StorageResult<()> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = receipts
            .iter()
            .map(|r| r.transaction_hash.as_bytes().to_vec())
            .collect();

        let fut = async move {
            let pb_receipts: Vec<SerReceipt> = receipts.into_iter().map(Into::into).collect();
            let values = await!(AsyncCodec::encode_batch(pb_receipts))?;

            await!(db
                .insert_batch(DataCategory::Receipt, keys, values)
                .compat())?;
            Ok(())
        };

        Box::new(Compat::new(fut))
    }
}

fn transfrom_u64_to_array_u8(n: u64) -> Vec<u8> {
    let mut u64_slice = [0u8; 8];
    NativeEndian::write_u64(&mut u64_slice, n);
    u64_slice.to_vec()
}

fn opts_to_flat(values: Vec<Option<Vec<u8>>>) -> Vec<Vec<u8>> {
    values
        .into_iter()
        .filter(Option::is_some)
        .map(|v| v.expect("get value"))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use old_futures::future::Future;

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

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_hash_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);

        let b = mock_block(1000);
        let hash = b.header.hash().clone();
        storage.insert_block(b).wait().unwrap();

        let b = storage.get_block_by_hash(&hash).wait().unwrap();
        assert_eq!(b.header.height, 1000)
    }

    #[test]
    fn test_get_transaction_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx = mock_transaction(Hash::digest(b"test111"));

        let hash = tx.hash.clone();
        storage.insert_transactions(vec![tx]).wait().unwrap();
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
        storage.insert_transactions(vec![tx1, tx2]).wait().unwrap();
        let transactions = storage
            .get_transactions(&[tx_hash1.clone(), tx_hash2.clone()])
            .wait()
            .unwrap();
        assert_eq!(transactions.len(), 2);

        let hashes: Vec<Hash> = transactions.into_iter().map(|tx| tx.hash).collect();

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

        assert_eq!(new_tx_position, tx_position);
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
            .get_transaction_positions(&[hash1, hash2])
            .wait()
            .unwrap();
        assert_eq!(tx_positions.len(), 2);

        assert!(tx_positions.contains(&tx_position1));
        assert!(tx_positions.contains(&tx_position2));
    }

    #[test]
    fn test_get_receipt_should_return_ok() {
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let receipt = mock_receipt(Hash::digest(b"test111"));
        let tx_hash = receipt.transaction_hash.clone();

        storage.insert_receipts(vec![receipt]).wait().unwrap();
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
            .insert_receipts(vec![receipt1, receipt2])
            .wait()
            .unwrap();
        let transactions = storage
            .get_receipts(&[tx_hash1.clone(), tx_hash2.clone()])
            .wait()
            .unwrap();
        assert_eq!(transactions.len(), 2);

        let hashes: Vec<Hash> = transactions
            .into_iter()
            .map(|receipt| receipt.transaction_hash)
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
                .header
                .height,
            height
        );

        assert_eq!(
            storage
                .get_block_by_hash(&hash)
                .wait()
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
