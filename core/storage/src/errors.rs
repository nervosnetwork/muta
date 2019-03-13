use std::error::Error;
use std::fmt;

use prost::{DecodeError, EncodeError};

use core_runtime::DatabaseError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    Database(DatabaseError),
    Decode(DecodeError),
    Encode(EncodeError),
    Internal,
}

impl Error for StorageError {
    fn description(&self) -> &str {
        match *self {
            StorageError::Database(_) => "database error",
            StorageError::Decode(_) => "decode error",
            StorageError::Encode(_) => "encode error",
            StorageError::Internal => "internal error",
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            StorageError::Database(ref err) => format!("database error: {:?}", err),
            StorageError::Decode(ref err) => format!("decode error: {:?}", err),
            StorageError::Encode(ref err) => format!("encode error: {:?}", err),
            StorageError::Internal => "internal error".to_string(),
        };
        write!(f, "{}", printable)
    }
}

impl From<DatabaseError> for StorageError {
    fn from(err: DatabaseError) -> Self {
        StorageError::Database(err)
    }
}

impl From<DecodeError> for StorageError {
    fn from(err: DecodeError) -> Self {
        StorageError::Decode(err)
    }
}

impl From<EncodeError> for StorageError {
    fn from(err: EncodeError) -> Self {
        StorageError::Encode(err)
    }
}
