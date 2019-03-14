use byteorder::{ByteOrder, NativeEndian};
use bytes::{BytesMut, IntoBuf};
use futures::future::{join_all, result, Future};
use prost::Message;

use core_runtime::{DatabaseFactory, DatabaseInstance, FutRuntimeResult};
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

pub struct BlockStorage<F>
where
    F: DatabaseFactory,
{
    factory: F,
}

impl<F> BlockStorage<F>
where
    F: DatabaseFactory,
{
    pub fn new(factory: F) -> Self {
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
                    .get(PREFIX_LATEST_BLOCK)
                    .map_err(StorageError::Database)
            })
            .and_then(AsyncCodec::decode::<PbBlock>)
            .map(Block::from);

        Box::new(fut)
    }

    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Block, StorageError> {
        let key = gen_key_with_u64(PREFIX_BLOCK_HEIGHT, height);

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| db_instance.get(&key).map_err(StorageError::Database))
            .and_then(AsyncCodec::decode::<PbBlock>)
            .map(Block::from);;

        Box::new(fut)
    }

    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Block, StorageError> {
        let key = gen_key_with_slice(PREFIX_BLOCK_HEIGHT_BY_HASH, hash.as_ref());

        let instance_fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database);

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| db_instance.get(&key).map_err(StorageError::Database))
            .map(|height_slice| transfrom_array_u8_to_u64(&height_slice))
            .join(instance_fut)
            .and_then(|(height, db_instance)| {
                db_instance
                    .get(&gen_key_with_u64(PREFIX_BLOCK_HEIGHT, height))
                    .map_err(StorageError::Database)
            })
            .and_then(AsyncCodec::decode::<PbBlock>)
            .map(Block::from);

        Box::new(fut)
    }

    fn get_transaction(&self, hash: &Hash) -> FutRuntimeResult<SignedTransaction, StorageError> {
        let key = gen_key_with_slice(PREFIX_TRANSACTION, hash.as_ref());

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| db_instance.get(&key).map_err(StorageError::Database))
            .and_then(AsyncCodec::decode::<PbSignedTransaction>)
            .map(SignedTransaction::from);

        Box::new(fut)
    }

    fn get_transactions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<SignedTransaction>>, StorageError> {
        let mut keys = vec![];
        for h in hashes {
            keys.push([PREFIX_TRANSACTION, h.as_ref()].concat());
        }

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| {
                db_instance.get_batch(&keys).map_err(StorageError::Database)
            })
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
        let key = gen_key_with_slice(PREFIX_RECEIPT, hash.as_ref());

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| db_instance.get(&key).map_err(StorageError::Database))
            .and_then(AsyncCodec::decode::<PbReceipt>)
            .map(Receipt::from);

        Box::new(fut)
    }

    fn get_receipts(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<Receipt>>, StorageError> {
        let mut keys = vec![];
        for h in hashes {
            keys.push([PREFIX_RECEIPT, h.as_ref()].concat());
        }

        let fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database)
            .and_then(move |db_instance| {
                db_instance.get_batch(&keys).map_err(StorageError::Database)
            })
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
        let instance_fut = self
            .factory
            .crate_instance()
            .map_err(StorageError::Database);

        let pb_block: PbBlock = block.clone().into();
        let mut encoded_buf = BytesMut::with_capacity(pb_block.encoded_len());

        let height = block.header.height;
        let height_key = gen_key_with_u64(PREFIX_BLOCK_HEIGHT, block.header.height);
        let hash_key = gen_key_with_slice(PREFIX_BLOCK_HEIGHT_BY_HASH, block.hash().as_ref());

        let fut = AsyncCodec::encode(pb_block, &mut encoded_buf)
            .and_then(|()| instance_fut)
            .and_then(move |mut db_instance| {
                join_all(vec![
                    db_instance
                        .insert(&height_key, encoded_buf.as_ref())
                        .map_err(StorageError::Database),
                    db_instance
                        .insert(&hash_key, &transfrom_u64_to_array_u8(height))
                        .map_err(StorageError::Database),
                    db_instance
                        .insert(PREFIX_LATEST_BLOCK, encoded_buf.as_ref())
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
            let mut buf = BytesMut::with_capacity(pb_tx.encoded_len());
            let fut = AsyncCodec::encode(pb_tx, &mut buf).map(move |_| buf.to_vec());

            keys.push(gen_key_with_slice(PREFIX_TRANSACTION, tx.hash.as_ref()));
            peding_fut.push(fut);
        }

        let fut =
            join_all(peding_fut)
                .join(instance_fut)
                .and_then(move |(buf_list, mut db_instance)| {
                    db_instance
                        .insert_batch(&keys, &buf_list)
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
            let mut buf = BytesMut::with_capacity(pb_receipt.encoded_len());
            let fut = AsyncCodec::encode(pb_receipt, &mut buf).map(move |_| buf.to_vec());

            keys.push(gen_key_with_slice(
                PREFIX_RECEIPT,
                receipt.transaction_hash.as_ref(),
            ));
            peding_fut.push(fut);
        }

        let fut =
            join_all(peding_fut)
                .join(instance_fut)
                .and_then(move |(buf_list, mut db_instance)| {
                    db_instance
                        .insert_batch(&keys, &buf_list)
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
    let mut u64_slice = [0u8; 8];
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

    pub fn encode<T: Message>(
        msg: T,
        mut buf: &mut BytesMut,
    ) -> FutRuntimeResult<(), StorageError> {
        Box::new(result(msg.encode(&mut buf).map_err(StorageError::Encode)))
    }
}

#[cfg(test)]
mod tests {
    use futures::future::Future;

    use super::{BlockStorage, Storage};

    use components_database::memory::Factory;
    use core_types::{Block, Hash, Receipt, SignedTransaction, UnverifiedTransaction};

    #[test]
    fn test_get_latest_block_should_return_ok() {
        let factory = Factory::new();
        let mut storage = BlockStorage::new(factory);
        storage.insert_block(&mock_block(1000)).wait().unwrap();
        let block = storage.get_latest_block().wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_height_should_return_ok() {
        let factory = Factory::new();
        let mut storage = BlockStorage::new(factory);
        storage.insert_block(&mock_block(1000)).wait().unwrap();
        let block = storage.get_block_by_height(1000).wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_hash_should_return_ok() {
        let factory = Factory::new();
        let mut storage = BlockStorage::new(factory);

        let b = mock_block(1000);
        storage.insert_block(&b).wait().unwrap();

        let b = storage.get_block_by_hash(&b.hash()).wait().unwrap();
        assert_eq!(b.header.height, 1000)
    }

    #[test]
    fn test_get_transaction_should_return_ok() {
        let factory = Factory::new();
        let mut storage = BlockStorage::new(factory);
        let tx = mock_transaction(Hash::from_raw(b"test111"));

        let hash = tx.hash.clone();
        storage.insert_transactions(&[tx]).wait().unwrap();
        let new_tx = storage.get_transaction(&hash).wait().unwrap();

        assert_eq!(new_tx.hash, hash)
    }

    #[test]
    fn test_get_transactions_should_return_ok() {
        let factory = Factory::new();
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
        let factory = Factory::new();
        let mut storage = BlockStorage::new(factory);
        let receipt = mock_receipt(Hash::from_raw(b"test111"));
        let tx_hash = receipt.transaction_hash.clone();

        storage.insert_receipts(&[receipt]).wait().unwrap();
        let receipt = storage.get_receipt(&tx_hash).wait().unwrap();
        assert_eq!(receipt.transaction_hash, tx_hash);
    }

    #[test]
    fn test_get_receipts_should_return_ok() {
        let factory = Factory::new();
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
        let factory = Factory::new();
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
