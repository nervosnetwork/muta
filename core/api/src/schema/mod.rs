mod epoch;
mod transaction;

pub use epoch::{Epoch, EpochHeader};
pub use transaction::{
    ContractType, InputDeployAction, InputRawTransaction, InputTransactionEncryption,
    InputTransferAction,
};

#[derive(GraphQLScalarValue, Clone)]
#[graphql(description = "The output digest of Keccak hash function")]
pub struct Hash(String);
pub type MerkleRoot = Hash;
pub type AssetID = Hash;

#[derive(GraphQLScalarValue, Clone)]
#[graphql(description = "21 bytes of account address, the first bytes of which is the identifier.")]
pub struct Address(String);

#[derive(GraphQLScalarValue, Clone)]
#[graphql(description = "Uint64")]
pub struct Uint64(String);

#[derive(GraphQLScalarValue, Clone)]
#[graphql(description = "uint256")]
pub struct Balance(String);

#[derive(GraphQLScalarValue, Clone)]
#[graphql(description = "Bytes corresponding hex string.")]
pub struct Bytes(String);

#[derive(GraphQLObject, Clone)]
#[graphql(description = "Transaction fee")]
pub struct Fee {
    asset_id: AssetID,
    cycle:    Uint64,
}

impl Hash {
    pub fn as_hex(&self) -> String {
        clean_0x(&self.0).to_owned().to_uppercase()
    }
}

impl Address {
    pub fn as_hex(&self) -> String {
        clean_0x(&self.0).to_owned().to_uppercase()
    }
}

impl Uint64 {
    pub fn as_hex(&self) -> String {
        clean_0x(&self.0).to_owned().to_uppercase()
    }
}

impl Balance {
    pub fn as_hex(&self) -> String {
        clean_0x(&self.0).to_owned().to_uppercase()
    }
}

impl Bytes {
    pub fn as_hex(&self) -> String {
        clean_0x(&self.0).to_owned().to_uppercase()
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
        Uint64(hex::encode(n.to_be_bytes().to_vec()))
    }
}

impl From<protocol::types::Balance> for Balance {
    fn from(balance: protocol::types::Balance) -> Self {
        Balance(hex::encode(balance.to_bytes_be().to_vec()))
    }
}

impl From<protocol::types::Fee> for Fee {
    fn from(fee: protocol::types::Fee) -> Self {
        Fee {
            asset_id: AssetID::from(fee.asset_id),
            cycle:    Uint64::from(fee.cycle),
        }
    }
}

impl From<bytes::Bytes> for Bytes {
    fn from(bytes: bytes::Bytes) -> Self {
        Bytes(hex::encode(bytes))
    }
}

fn clean_0x(s: &str) -> &str {
    if s.starts_with("0x") {
        &s[2..]
    } else {
        s
    }
}
