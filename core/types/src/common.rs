use std::convert::{AsRef, From};
use std::fmt;

use crate::errors::CoreTypesError;
use numext_fixed_hash::H160;
use numext_fixed_uint::U256;
use rlp::{Encodable, RlpStream};
use sha3::{Digest, Sha3_256};

const ADDRESS_LEN: usize = 20;
const HASH_LEN: usize = 32;

pub type Balance = U256;
pub type H256 = numext_fixed_hash::H256;

/// Address represents the 20 byte address of an cita account.
#[derive(Default, Clone, PartialEq, Eq, Hash)]
pub struct Address(H160);

impl Address {
    pub fn from_hash(h: &Hash) -> Self {
        Address::from(&h.as_ref()[12..])
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0.as_bytes())
    }

    pub fn from_hex(input: &str) -> Result<Self, CoreTypesError> {
        Ok(Address(H160::from_hex_str(input)?))
    }

    pub fn as_fixed_bytes(&self) -> &[u8; ADDRESS_LEN] {
        self.0.as_fixed_bytes()
    }

    pub fn into_fixed_bytes(self) -> [u8; ADDRESS_LEN] {
        self.0.into_fixed_bytes()
    }
}

impl From<[u8; ADDRESS_LEN]> for Address {
    fn from(data: [u8; ADDRESS_LEN]) -> Self {
        Address(H160::from(data))
    }
}

impl From<&[u8]> for Address {
    fn from(data: &[u8]) -> Self {
        let mut arr = [0u8; 20];
        arr.copy_from_slice(&data[..]);
        Address(H160::from(arr))
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
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
    pub fn from_raw(raw: &[u8]) -> Self {
        let mut out = [0u8; HASH_LEN];
        if raw.len() == HASH_LEN {
            out.copy_from_slice(raw);
            Hash(H256::from(out))
        } else {
            out.copy_from_slice(&Sha3_256::digest(raw));
            Hash(H256::from(out))
        }
    }

    pub fn from_hex(input: &str) -> Result<Self, CoreTypesError> {
        Ok(Hash(H256::from_hex_str(input)?))
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

impl From<[u8; HASH_LEN]> for Hash {
    fn from(data: [u8; HASH_LEN]) -> Self {
        let hash = H256::from(data);
        Hash(hash)
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
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
