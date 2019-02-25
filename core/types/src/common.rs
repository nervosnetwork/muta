use std::convert::From;
use std::fmt;

use numext_fixed_hash::{H160, H256};
use sha3::{Digest, Sha3_256};

const ADDRESS_LEN: usize = 20;
const HASH_LEN: usize = 32;

/// Address represents the 20 byte address of an cita account.
#[derive(Default, Clone, PartialEq)]
pub struct Address(H160);

impl Address {
    pub fn as_hex(&self) -> String {
        hex::encode(self.0.as_bytes())
    }
}

impl From<[u8; ADDRESS_LEN]> for Address {
    fn from(data: [u8; ADDRESS_LEN]) -> Self {
        Address(H160::from(data))
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0.as_bytes()))
    }
}

/// Hash represents the 32 byte sha3-256 hash of arbitrary data.
#[derive(Default, Clone, PartialEq)]
pub struct Hash(H256);

impl Hash {
    pub fn from_raw(raw: &[u8]) -> Self {
        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&Sha3_256::digest(raw));
        Hash(H256::from(out))
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0.as_bytes())
    }
}

impl From<[u8; HASH_LEN]> for Hash {
    fn from(data: [u8; HASH_LEN]) -> Self {
        let hash = H256::from(data);
        Hash(hash)
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0.as_bytes()))
    }
}
