use std::error::Error;
use std::fmt;

use crate::FutRuntimeResult;

/// Specify the category of data stored, and users can store the data in a decentralized manner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataCategory {
    // Block
    Block,
    // Already of "SignedTransaction" in the block.
    Transaction,
    // Already of "Receipt" in the block.
    Receipt,
    // State of the world
    State,
    // "SignedTransaction" in the transaction pool
    TransactionPool,
    // Transaction position in block
    TransactionPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseError {
    NotFound,
    InvalidData,
    Internal(String),
}

pub type FutDBResult<T> = FutRuntimeResult<T, DatabaseError>;

pub trait Database: Send + Sync {
    fn get(&self, c: DataCategory, key: &[u8]) -> FutDBResult<Option<Vec<u8>>>;

    fn get_batch(&self, c: DataCategory, keys: &[Vec<u8>]) -> FutDBResult<Vec<Option<Vec<u8>>>>;

    fn insert(&self, c: DataCategory, key: Vec<u8>, value: Vec<u8>) -> FutDBResult<()>;

    fn insert_batch(
        &self,
        c: DataCategory,
        keys: Vec<Vec<u8>>,
        values: Vec<Vec<u8>>,
    ) -> FutDBResult<()>;

    fn contains(&self, c: DataCategory, key: &[u8]) -> FutDBResult<bool>;

    fn remove(&self, c: DataCategory, key: &[u8]) -> FutDBResult<()>;

    fn remove_batch(&self, c: DataCategory, keys: &[Vec<u8>]) -> FutDBResult<()>;
}

impl Error for DatabaseError {}
impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            DatabaseError::NotFound => "not found".to_owned(),
            DatabaseError::InvalidData => "invalid data".to_owned(),
            DatabaseError::Internal(ref err) => format!("internal error: {:?}", err),
        };
        write!(f, "{}", printable)
    }
}
