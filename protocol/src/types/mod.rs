pub(crate) mod block;
pub(crate) mod genesis;
pub(crate) mod primitive;
pub(crate) mod receipt;
pub(crate) mod service_context;
pub(crate) mod transaction;

use std::error::Error;

use derive_more::{Display, From};

use crate::{ProtocolError, ProtocolErrorKind};

pub use block::{Block, BlockHeader, Pill, Proof, Validator};
pub use bytes::{Bytes, BytesMut};
pub use ethbloom::{Bloom, BloomRef, Input as BloomInput};
pub use genesis::{Genesis, ServiceParam};
pub use primitive::{
    Address, Hash, Hex, JsonString, MerkleRoot, Metadata, ValidatorExtend, GENESIS_HEIGHT,
    METADATA_KEY,
};
pub use receipt::{Event, Receipt, ReceiptResponse};
pub use service_context::{ServiceContext, ServiceContextError, ServiceContextParams};
pub use transaction::{RawTransaction, SignedTransaction, TransactionRequest};

#[derive(Debug, Display, From)]
pub enum TypesError {
    #[display(fmt = "Expect {:?}, get {:?}.", expect, real)]
    LengthMismatch { expect: usize, real: usize },

    #[display(fmt = "{:?}", error)]
    FromHex { error: hex::FromHexError },

    #[display(fmt = "{:?} is an invalid address", address)]
    InvalidAddress { address: String },

    #[display(fmt = "Hex should start with 0x")]
    HexPrefix,

    #[display(fmt = "Invalid Witness")]
    InvalidWitness,
}

impl Error for TypesError {}

impl From<TypesError> for ProtocolError {
    fn from(error: TypesError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Types, Box::new(error))
    }
}
