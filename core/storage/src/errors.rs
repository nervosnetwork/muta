use std::error::Error;
use std::fmt;

use core_runtime::DatabaseError;
use core_runtime::FutRuntimeResult;
use core_serialization::CodecError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    Database(DatabaseError),
    Codec(CodecError),
    Internal(String),
}

pub type StorageResult<T> = FutRuntimeResult<T, StorageError>;

impl StorageError {
    pub fn is_database_not_found(err: StorageError) -> bool {
        match err {
            StorageError::Database(err) => match err {
                DatabaseError::NotFound => true,
                _ => false,
            },
            _ => false,
        }
    }
}

impl Error for StorageError {}
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            StorageError::Database(ref err) => format!("database error: {:?}", err),
            StorageError::Codec(ref err) => format!("codec error: {:?}", err),
            StorageError::Internal(ref err) => format!("internal error: {:?}", err),
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
