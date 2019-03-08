use byteorder::{ByteOrder, NativeEndian};
use bytes::{BytesMut, IntoBuf};
use futures::future::{err, ok, result, Future};
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
    db: DB,
}

impl<DB> BlockStorage<DB>
where
    DB: Database,
{
    pub fn new(db: DB) -> Self {
        BlockStorage { db }
    }
}

impl<DB> Storage for BlockStorage<DB>
where
    DB: Database,
{
    fn get_latest_block(&self) -> FutRuntimeResult<Block, StorageError> {
        let fut = self
            .db
            .get(PREFIX_LATEST_BLOCK)
            .from_err()
            .and_then(|data| result(PbBlock::decode(data.into_buf()).map_err(StorageError::Decode)))
            .from_err()
            .and_then(|b| ok(Block::from(b)));

        Box::new(fut)
    }

    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Block, StorageError> {
        let fut = self
            .db
            .get(&gen_key_with_u64(PREFIX_BLOCK_HEIGHT, height))
            .from_err()
            .and_then(|data| result(PbBlock::decode(data.into_buf()).map_err(StorageError::Decode)))
            .from_err()
            .and_then(|b| ok(Block::from(b)));

        Box::new(fut)
    }

    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Block, StorageError> {
        let result: Result<u64, StorageError> = self
            .db
            .get(&gen_key_with_slice(
                PREFIX_BLOCK_HEIGHT_BY_HASH,
                hash.as_ref(),
            ))
            .map_err(StorageError::Database)
            .and_then(|height_slice| ok(transfrom_array_u8_to_u64(&height_slice)))
            .wait();

        match result {
            Ok(height) => self.get_block_by_height(height),
            Err(e) => Box::new(err::<Block, StorageError>(e)),
        }
    }

    fn get_transaction(&self, hash: &Hash) -> FutRuntimeResult<SignedTransaction, StorageError> {
        let fut = self
            .db
            .get(&gen_key_with_slice(PREFIX_TRANSACTION, hash.as_ref()))
            .from_err()
            .and_then(|data| {
                result(PbSignedTransaction::decode(data.into_buf()).map_err(StorageError::Decode))
            })
            .from_err()
            .and_then(|tx| ok(SignedTransaction::from(tx)));

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
            .db
            .get_batch(&keys)
            .from_err()
            .and_then(|opt_txs_data| {
                let results: Vec<Result<PbSignedTransaction, StorageError>> = opt_txs_data
                    .iter()
                    .filter_map(Option::as_ref)
                    .map(|data| {
                        PbSignedTransaction::decode(data.into_buf()).map_err(StorageError::Decode)
                    })
                    .collect();

                ok(results
                    .into_iter()
                    .map(|result| match result {
                        Ok(tx) => Some(SignedTransaction::from(tx)),
                        Err(e) => {
                            // FIX: Replace log.
                            println!("get_transactions error {:?}", e);
                            None
                        }
                    })
                    .collect())
            });

        Box::new(fut)
    }

    fn get_receipt(&self, hash: &Hash) -> FutRuntimeResult<Receipt, StorageError> {
        let fut = self
            .db
            .get(&gen_key_with_slice(PREFIX_RECEIPT, hash.as_ref()))
            .from_err()
            .and_then(|data| {
                result(PbReceipt::decode(data.into_buf())).map_err(StorageError::Decode)
            })
            .and_then(|r| ok(Receipt::from(r)));

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
            .db
            .get_batch(&keys)
            .from_err()
            .and_then(|opt_txs_data| {
                let results: Vec<Result<PbReceipt, StorageError>> = opt_txs_data
                    .iter()
                    .filter_map(Option::as_ref)
                    .map(|data| PbReceipt::decode(data.into_buf()).map_err(StorageError::Decode))
                    .collect();

                ok(results
                    .into_iter()
                    .map(|result| match result {
                        Ok(r) => Some(Receipt::from(r)),
                        Err(e) => {
                            // FIX: Replace log.
                            println!("get_receipts error {:?}", e);
                            None
                        }
                    })
                    .collect())
            });

        Box::new(fut)
    }

    fn insert_block(&mut self, block: &Block) -> FutRuntimeResult<(), StorageError> {
        let pb_block: PbBlock = block.clone().into();
        let mut b = BytesMut::new();

        // TODO: Can someone teach me how to reduce code nesting here?
        match pb_block.encode(&mut b).map_err(StorageError::Encode) {
            Err(e) => Box::new(err(e)),
            Ok(()) => {
                // First insert a block by height.
                let key = &gen_key_with_u64(PREFIX_BLOCK_HEIGHT, block.header.height);
                match self
                    .db
                    .insert(key, b.as_ref())
                    .map_err(StorageError::Database)
                    .wait()
                {
                    Err(e) => Box::new(err(e)),
                    Ok(()) => {
                        // Then insert a hash index.
                        let key =
                            &gen_key_with_slice(PREFIX_BLOCK_HEIGHT_BY_HASH, block.hash().as_ref());
                        Box::new(
                            self.db
                                .insert(key, &transfrom_u64_to_array_u8(block.header.height))
                                .map_err(StorageError::Database),
                        )
                    }
                }
            }
        }
    }

    fn insert_transactions(
        &mut self,
        signed_txs: &[SignedTransaction],
    ) -> FutRuntimeResult<(), StorageError> {
        let mut keys = vec![];
        let mut values = vec![];

        for signed_tx in signed_txs {
            let pb_signed_tx: PbSignedTransaction = signed_tx.clone().into();
            let mut b = BytesMut::new();

            match pb_signed_tx.encode(&mut b).map_err(StorageError::Encode) {
                Err(e) => return Box::new(err(e)),
                Ok(()) => {
                    keys.push(gen_key_with_slice(
                        PREFIX_TRANSACTION,
                        signed_tx.hash.as_ref(),
                    ));
                    values.push(b.as_ref().to_vec());
                }
            }
        }

        Box::new(
            self.db
                .insert_batch(&keys, &values)
                .map_err(StorageError::Database),
        )
    }

    fn insert_receipts(&mut self, receipts: &[Receipt]) -> FutRuntimeResult<(), StorageError> {
        let mut keys = vec![];
        let mut values = vec![];

        for receipt in receipts {
            let pb_receipt: PbReceipt = receipt.clone().into();
            let mut b = BytesMut::new();

            match pb_receipt.encode(&mut b).map_err(StorageError::Encode) {
                Err(e) => return Box::new(err(e)),
                Ok(()) => {
                    keys.push(gen_key_with_slice(
                        PREFIX_RECEIPT,
                        &receipt.transaction_hash.as_ref().to_vec(),
                    ));
                    values.push(b.as_ref().to_vec());
                }
            }
        }

        Box::new(
            self.db
                .insert_batch(&keys, &values)
                .map_err(StorageError::Database),
        )
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
