use core_runtime::{Database, FutRuntimeResult};
use core_types::{Block, Hash, Receipt, SignedTransaction};

use crate::errors::StorageError;

// TODO: remove these 'allow(dead_code)'
#[allow(dead_code)]
const PREFIX_LATEST_BLOCK: &[u8] = b"latest-block";
#[allow(dead_code)]
const PREFIX_BLOCK_HEIGHT_BY_HASH: &[u8] = b"block-hash-";
#[allow(dead_code)]
const PREFIX_BLOCK_HEIGHT: &[u8] = b"block-height-";
#[allow(dead_code)]
const PREFIX_TRANSACTION: &[u8] = b"transaction-";
#[allow(dead_code)]
const PREFIX_RECEIPT: &[u8] = b"receipt-";

/// "storage" is responsible for the storage and retrieval of blockchain data.
/// Block, transaction, and receipt can be obtained from here,
/// but data related to "world status" is not available.
pub trait Storage: Send + Sync {
    type Error;

    /// Get the latest block.
    fn get_latest_block(&self) -> FutRuntimeResult<Option<Block>, Self::Error>;

    /// Get a block by height.
    fn get_block_by_height(&self, height: u64) -> FutRuntimeResult<Option<Block>, Self::Error>;

    /// Get a block by hash.
    /// The hash is actually an index,
    /// and the corresponding height is obtained by hash and then querying the corresponding block.
    fn get_block_by_hash(&self, hash: &Hash) -> FutRuntimeResult<Option<Block>, Self::Error>;

    /// Get a signed transaction by hash.
    fn get_transaction(
        &self,
        hash: &Hash,
    ) -> FutRuntimeResult<Option<SignedTransaction>, Self::Error>;

    /// Get a batch of transactions by hashes.
    fn get_transactions(
        &self,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<SignedTransaction>>, Self::Error>;

    /// Get a receipt by hash.
    fn get_receipt(&self, tx_hash: &Hash) -> FutRuntimeResult<Option<Receipt>, Self::Error>;

    /// Get a batch of receipts by hashes
    fn get_receipts(
        &self,
        tx_hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<Receipt>>, Self::Error>;

    /// Insert a block.
    fn insert_block(&self, block: &Block) -> FutRuntimeResult<(), Self::Error>;

    /// Insert a batch of transactions.
    fn insert_transactions(
        &self,
        signed_txs: &[&SignedTransaction],
    ) -> FutRuntimeResult<(), Self::Error>;

    /// Insert a batch of receipts.
    fn insert_receipts(&self, receipts: &[&Receipt]) -> FutRuntimeResult<(), Self::Error>;
}

// TODO: remove this
#[allow(dead_code)]
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
    type Error = StorageError;

    fn get_latest_block(&self) -> FutRuntimeResult<Option<Block>, Self::Error> {
        // self.db.insert(PREFIX_LATEST_BLOCK, value: &[u8])
        unimplemented!()
    }

    fn get_block_by_height(&self, _height: u64) -> FutRuntimeResult<Option<Block>, Self::Error> {
        unimplemented!()
    }

    fn get_block_by_hash(&self, _hash: &Hash) -> FutRuntimeResult<Option<Block>, Self::Error> {
        unimplemented!()
    }

    fn get_transaction(
        &self,
        _hash: &Hash,
    ) -> FutRuntimeResult<Option<SignedTransaction>, Self::Error> {
        unimplemented!()
    }

    fn get_transactions(
        &self,
        _hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<SignedTransaction>>, Self::Error> {
        unimplemented!()
    }

    fn get_receipt(&self, _hash: &Hash) -> FutRuntimeResult<Option<Receipt>, Self::Error> {
        unimplemented!()
    }

    fn get_receipts(
        &self,
        _hashes: &[&Hash],
    ) -> FutRuntimeResult<Vec<Option<Receipt>>, Self::Error> {
        unimplemented!()
    }

    fn insert_block(&self, _block: &Block) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn insert_transactions(
        &self,
        _signed_txs: &[&SignedTransaction],
    ) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn insert_receipts(&self, _receipts: &[&Receipt]) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }
}
