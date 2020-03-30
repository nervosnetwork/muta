mod block;
mod receipt;
mod transaction;

use std::convert::From;

use derive_more::{Display, From};
use std::num::ParseIntError;

use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub use block::{Block, BlockHeader};
pub use receipt::{Event, Receipt, ReceiptResponse};
pub use transaction::{
    to_signed_transaction, to_transaction, InputRawTransaction, InputTransactionEncryption,
    SignedTransaction,
};

#[derive(juniper::GraphQLObject, Clone)]
pub struct ServiceResponse {
    pub code:          Uint64,
    pub succeed_data:  String,
    pub error_message: String,
}

impl From<protocol::traits::ServiceResponse<String>> for ServiceResponse {
    fn from(resp: protocol::traits::ServiceResponse<String>) -> Self {
        Self {
            code:          Uint64::from(resp.code),
            succeed_data:  resp.succeed_data,
            error_message: resp.error_message,
        }
    }
}

#[derive(juniper::GraphQLScalarValue, Clone)]
#[graphql(description = "The output digest of Keccak hash function")]
pub struct Hash(String);
pub type MerkleRoot = Hash;

#[derive(juniper::GraphQLScalarValue, Clone)]
#[graphql(description = "20 bytes of account address")]
pub struct Address(String);

#[derive(juniper::GraphQLScalarValue, Clone)]
#[graphql(description = "Uint64")]
pub struct Uint64(String);

#[derive(juniper::GraphQLScalarValue, Clone)]
#[graphql(description = "Bytes corresponding hex string.")]
pub struct Bytes(String);

impl Hash {
    pub fn as_hex(&self) -> String {
        self.0.to_uppercase()
    }
}

impl Address {
    pub fn as_hex(&self) -> String {
        self.0.to_uppercase()
    }
}

impl Uint64 {
    pub fn as_hex(&self) -> ProtocolResult<String> {
        Ok(clean_0x(&self.0)?.to_uppercase())
    }

    pub fn try_into_u64(self) -> ProtocolResult<u64> {
        let n = u64::from_str_radix(&self.as_hex()?, 16).map_err(SchemaError::IntoU64)?;
        Ok(n)
    }
}

impl Bytes {
    pub fn as_hex(&self) -> ProtocolResult<String> {
        Ok(clean_0x(&self.0)?.to_uppercase())
    }

    pub fn to_vec(&self) -> ProtocolResult<Vec<u8>> {
        let v = hex::decode(self.as_hex()?).map_err(SchemaError::FromHex)?;
        Ok(v)
    }
}

impl From<protocol::types::Hash> for Hash {
    fn from(hash: protocol::types::Hash) -> Self {
        Hash(hash.as_hex())
    }
}

impl From<protocol::types::Address> for Address {
    fn from(address: protocol::types::Address) -> Self {
        Address(address.as_hex())
    }
}

impl From<u64> for Uint64 {
    fn from(n: u64) -> Self {
        Uint64("0x".to_owned() + &hex::encode(n.to_be_bytes().to_vec()))
    }
}

impl From<protocol::Bytes> for Bytes {
    fn from(bytes: protocol::Bytes) -> Self {
        Bytes("0x".to_owned() + &hex::encode(bytes))
    }
}

fn clean_0x(s: &str) -> ProtocolResult<String> {
    if s.starts_with("0x") || s.starts_with("0X") {
        Ok(s[2..].to_owned())
    } else {
        Err(SchemaError::HexPrefix.into())
    }
}

#[derive(Debug, Display, From)]
pub enum SchemaError {
    #[display(fmt = "into u64 {:?}", _0)]
    IntoU64(ParseIntError),

    #[display(fmt = "from hex {:?}", _0)]
    FromHex(hex::FromHexError),

    #[display(fmt = "hex should start with 0x")]
    HexPrefix,
}

impl std::error::Error for SchemaError {}

impl From<SchemaError> for ProtocolError {
    fn from(err: SchemaError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::API, Box::new(err))
    }
}
