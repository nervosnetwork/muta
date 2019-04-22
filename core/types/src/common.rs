use std::fmt;

use numext_fixed_hash::H160;
use numext_fixed_uint::U256;
use rlp::{Encodable, RlpStream};
use serde::{Serialize, Serializer};
use sha3::{Digest, Sha3_256};

use crate::errors::TypesError;

const ADDRESS_LEN: usize = 20;
const HASH_LEN: usize = 32;

pub type Balance = U256;
pub type H256 = numext_fixed_hash::H256;

/// Address represents the 20 byte address of an cita account.
#[derive(Default, Clone, PartialEq, Eq, Hash)]
pub struct Address(H160);

impl Address {
    pub fn from_hash(h: &Hash) -> Self {
        let mut out = [0u8; 20];
        out.copy_from_slice(&h.as_bytes()[12..]);
        Address::from_fixed_bytes(out)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, TypesError> {
        if data.len() != ADDRESS_LEN {
            return Err(TypesError::AddressLenInvalid);
        }
        let mut out = [0u8; 20];
        out.copy_from_slice(&data[..]);
        Ok(Address(H160::from(out)))
    }

    pub fn from_fixed_bytes(data: [u8; ADDRESS_LEN]) -> Self {
        Address(H160::from(data))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0.as_bytes())
    }

    pub fn from_hex(input: &str) -> Result<Self, TypesError> {
        Ok(Address(H160::from_hex_str(input)?))
    }

    pub fn as_fixed_bytes(&self) -> &[u8; ADDRESS_LEN] {
        self.0.as_fixed_bytes()
    }

    pub fn into_fixed_bytes(self) -> [u8; ADDRESS_LEN] {
        self.0.into_fixed_bytes()
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0.as_bytes()))
    }
}

/// Structure encodable to RLP
impl Encodable for Address {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.0.as_bytes());
    }
}

/// Hash represents the 32 byte sha3-256 hash of arbitrary data.
#[derive(Default, Clone, PartialEq, Eq, Hash)]
pub struct Hash(H256);

impl Hash {
    /// NOTE: The hash for bytes is not computed.
    pub fn from_bytes(data: &[u8]) -> Result<Self, TypesError> {
        if data.len() != HASH_LEN {
            return Err(TypesError::HashLenInvalid);
        }

        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(data);
        Ok(Hash(H256::from(out)))
    }

    pub fn digest(raw: &[u8]) -> Self {
        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&Sha3_256::digest(raw));
        Hash(H256::from(out))
    }

    pub fn from_fixed_bytes(data: [u8; HASH_LEN]) -> Self {
        let hash = H256::from(data);
        Hash(hash)
    }

    pub fn from_hex(input: &str) -> Result<Self, TypesError> {
        Ok(Hash(H256::from_hex_str(input)?))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0.as_bytes())
    }

    pub fn as_fixed_bytes(&self) -> &[u8; HASH_LEN] {
        self.0.as_fixed_bytes()
    }

    pub fn into_fixed_bytes(self) -> [u8; HASH_LEN] {
        self.0.into_fixed_bytes()
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0.as_bytes()))
    }
}

/// Structure encodable to RLP
impl Encodable for Hash {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.0.as_bytes());
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_hex())
    }
}
