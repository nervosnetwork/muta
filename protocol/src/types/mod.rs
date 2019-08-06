mod primitive;
mod transaction;

use std::error::Error;
use std::fmt;

use crate::{ProtocolError, ProtocolErrorKind};

pub use primitive::{Address, Hash};
pub use transaction::{ContractType, Fee, RawTransaction, SignedTransaction, TransactionAction};

#[derive(Debug)]
pub enum TypesError {
    HashLengthMismatch { expect: usize, real: usize },
    FromHex { error: hex::FromHexError },
}

impl Error for TypesError {}

impl fmt::Display for TypesError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            TypesError::HashLengthMismatch { expect, real } => {
                format!("Expect {:?} to get {:?}.", expect, real)
            }
            TypesError::FromHex { error } => format!("{:?}.", error),
        };
        write!(f, "{}", printable)
    }
}
impl From<TypesError> for ProtocolError {
    fn from(error: TypesError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Types, Box::new(error))
    }
}

impl From<hex::FromHexError> for TypesError {
    fn from(error: hex::FromHexError) -> Self {
        TypesError::FromHex { error }
    }
}
