use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::option::NoneError;

use core_context::Context;
use core_serialization::CodecError;
use core_types::{Block, Hash, Proof, Receipt, SignedTransaction, TransactionPosition};

use crate::{BoxFuture, DatabaseError};

pub type StorageResult<T> = BoxFuture<'static, Result<T, StorageError>>;

/// "storage" is responsible for the storage and retrieval of blockchain data.
/// Block, transaction, and receipt can be obtained from here,
/// but data related to "world status" is not available.
/// NOTE: Anything that might return "std::option::None" will return
/// "std::option:: NoneError".
pub trait Storage: Send + Sync {
    /// Get the latest block.
    fn get_latest_block(&self, ctx: Context) -> StorageResult<Block>;

    /// Get a block by height.
    fn get_block_by_height(&self, ctx: Context, height: u64) -> StorageResult<Block>;

    /// Get a block by hash.
    /// The hash is actually an index,
    /// and the corresponding height is obtained by hash and then querying the
    /// corresponding block.
    fn get_block_by_hash(&self, ctx: Context, hash: &Hash) -> StorageResult<Block>;

    /// Get a signed transaction by hash.
    fn get_transaction(&self, ctx: Context, hash: &Hash) -> StorageResult<SignedTransaction>;

    /// Get a batch of transactions by hashes.
    fn get_transactions(
        &self,
        ctx: Context,
        hashes: &[Hash],
    ) -> StorageResult<Vec<SignedTransaction>>;

    /// Get a receipt by hash.
    fn get_receipt(&self, ctx: Context, tx_hash: &Hash) -> StorageResult<Receipt>;

    /// Get a batch of receipts by hashes
    fn get_receipts(&self, ctx: Context, tx_hashes: &[Hash]) -> StorageResult<Vec<Receipt>>;

    /// Get a transaction position by hash.
    fn get_transaction_position(
        &self,
        ctx: Context,
        hash: &Hash,
    ) -> StorageResult<TransactionPosition>;

    /// Get a batch of transactions by hashes.
    fn get_transaction_positions(
        &self,
        ctx: Context,
        hashes: &[Hash],
    ) -> StorageResult<Vec<TransactionPosition>>;

    // Get the latest proof.
    fn get_latest_proof(&self, ctx: Context) -> StorageResult<Proof>;

    /// Insert a block.
    fn insert_block(&self, ctx: Context, block: Block) -> StorageResult<()>;

    /// Insert a batch of transactions.
    fn insert_transactions(
        &self,
        ctx: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> StorageResult<()>;

    /// Insert a batch of transaction positions.
    fn insert_transaction_positions(
        &self,
        ctx: Context,
        positions: HashMap<Hash, TransactionPosition>,
    ) -> StorageResult<()>;

    /// Insert a batch of receipts.
    fn insert_receipts(&self, ctx: Context, receipts: Vec<Receipt>) -> StorageResult<()>;

    // Update the latest proof.
    fn update_latest_proof(&self, ctx: Context, proof: Proof) -> StorageResult<()>;
}

#[derive(Debug)]
pub enum StorageError {
    Database(DatabaseError),
    Codec(CodecError),
    Internal(String),
    None(NoneError),
}

impl Error for StorageError {}
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            StorageError::Database(ref err) => format!("database error: {:?}", err),
            StorageError::Codec(ref err) => format!("codec error: {:?}", err),
            StorageError::Internal(ref err) => format!("internal error: {:?}", err),
            StorageError::None(ref err) => format!("{:?}", err),
        };
        write!(f, "{}", printable)
    }
}

impl From<DatabaseError> for StorageError {
    fn from(err: DatabaseError) -> Self {
        StorageError::Database(err)
    }
}

impl From<CodecError> for StorageError {
    fn from(err: CodecError) -> Self {
        StorageError::Codec(err)
    }
}

impl From<String> for StorageError {
    fn from(err: String) -> Self {
        StorageError::Internal(err)
    }
}

impl From<NoneError> for StorageError {
    fn from(err: NoneError) -> Self {
        StorageError::None(err)
    }
}
