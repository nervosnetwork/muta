pub(crate) mod block;
pub(crate) mod genesis;
pub(crate) mod primitive;
pub(crate) mod receipt;
pub(crate) mod service_context;
pub(crate) mod service_meta;
pub(crate) mod transaction;

use std::error::Error;

use derive_more::{Display, From};

use crate::{ProtocolError, ProtocolErrorKind};

pub use block::{Block, BlockHeader, Pill, Proof, Validator};
pub use bytes::{Bytes, BytesMut};
pub use genesis::{Genesis, ServiceParam};
pub use primitive::{
    Address, ChainSchema, Hash, Hex, JsonString, MerkleRoot, Metadata, ServiceSchema,
    ValidatorExtend, GENESIS_HEIGHT, METADATA_KEY,
};
pub use receipt::{Event, Receipt, ReceiptResponse};
pub use service_context::{ServiceContext, ServiceContextError, ServiceContextParams};
pub use service_meta::{DataMeta, FieldMeta, MethodMeta, ScalarMeta, ServiceMeta, StructMeta};
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
}

impl Error for TypesError {}

impl From<TypesError> for ProtocolError {
    fn from(error: TypesError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Types, Box::new(error))
    }
}
