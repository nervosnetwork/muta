use std::fmt;

use bytes::{Bytes, BytesMut};
use hasher::{Hasher, HasherKeccak};
use lazy_static::lazy_static;
use num_bigint::BigUint;
use serde::de;
use serde::{Deserialize, Serialize};

use crate::types::TypesError;
use crate::ProtocolResult;

pub const METADATA_KEY: &str = "metadata";

lazy_static! {
    static ref HASHER_INST: HasherKeccak = HasherKeccak::new();
}

/// The height of the genesis block.
pub const GENESIS_EPOCH_ID: u64 = 0;

/// Hash length
const HASH_LEN: usize = 32;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash([u8; HASH_LEN]);
/// Balance
pub type Balance = BigUint;
/// Merkel root hash
pub type MerkleRoot = Hash;
/// Json string
pub type JsonString = String;

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&self.as_hex())
    }
}

struct HashVisitor;

impl<'de> de::Visitor<'de> for HashVisitor {
    type Value = Hash;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expect a hex string")
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Hash::from_hex(&v).map_err(|e| de::Error::custom(e.to_string()))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Hash::from_hex(&v).map_err(|e| de::Error::custom(e.to_string()))
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_string(HashVisitor)
    }
}

impl Hash {
    /// Enter an array of bytes to get a 32-bit hash.
    /// Note: sha3 is used for the time being and may be replaced with other
    /// hashing algorithms later.
    pub fn digest(bytes: Bytes) -> Self {
        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&HASHER_INST.digest(&bytes));

        Self(out)
    }

    pub fn from_empty() -> Self {
        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&HASHER_INST.digest(&rlp::NULL_RLP));

        Self(out)
    }

    /// Converts the byte array to a Hash type.
    /// Note: if you want to compute the hash value of the byte array, you
    /// should call `fn digest`.
    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        ensure_len(bytes.len(), HASH_LEN)?;

        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&bytes);
        Ok(Self(out))
    }

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s);
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let bytes = Bytes::from(bytes);
        Self::from_bytes(bytes)
    }

    pub fn as_bytes(&self) -> Bytes {
        BytesMut::from(self.0.as_ref()).freeze()
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl Default for Hash {
    fn default() -> Self {
        Hash::from_empty()
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_hex())
    }
}

/// Address length.
const ADDRESS_LEN: usize = 20;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address([u8; ADDRESS_LEN]);

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&self.as_hex())
    }
}

struct AddressVisitor;

impl<'de> de::Visitor<'de> for AddressVisitor {
    type Value = Address;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expect a hex string")
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Address::from_hex(&v).map_err(|e| de::Error::custom(e.to_string()))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Address::from_hex(&v).map_err(|e| de::Error::custom(e.to_string()))
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_string(AddressVisitor)
    }
}

impl Address {
    pub fn from_pubkey_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        let hash = Hash::digest(bytes);

        Self::from_hash(hash)
    }

    pub fn from_hash(hash: Hash) -> ProtocolResult<Self> {
        let mut hash_val = hash.as_bytes();
        hash_val.truncate(20);

        Self::from_bytes(hash_val)
    }

    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        ensure_len(bytes.len(), ADDRESS_LEN)?;

        let mut out = [0u8; ADDRESS_LEN];
        out.copy_from_slice(&bytes);
        Ok(Self(out))
    }

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s);
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let bytes = Bytes::from(bytes);
        Self::from_bytes(bytes)
    }

    pub fn as_bytes(&self) -> Bytes {
        BytesMut::from(self.0.as_ref()).freeze()
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_hex())
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Metadata {
    pub chain_id:           Hash,
    pub verifier_list:      Vec<Address>,
    pub consensus_interval: u64,
    pub cycles_limit:       u64,
    pub cycles_price:       u64,
}

fn clean_0x(s: &str) -> &str {
    if s.starts_with("0x") {
        &s[2..]
    } else {
        s
    }
}

fn ensure_len(real: usize, expect: usize) -> ProtocolResult<()> {
    if real != expect {
        Err(TypesError::LengthMismatch { expect, real }.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::{Address, Hash};

    #[test]
    fn test_hash() {
        let hash = Hash::digest(Bytes::from("xxxxxx"));

        let bytes = hash.as_bytes();
        Hash::from_bytes(bytes).unwrap();
    }

    #[test]
    fn test_from_pubkey_bytes() {
        let pubkey = "031313016e9670deb49779c1b0c646d6a25a545712658f9781995f623bcd0d0b3d";
        let expect_addr = "c38f8210896e11a75e1a1f13805d39088d157d7f";

        let pubkey_bytes = Bytes::from(hex::decode(pubkey).unwrap());
        let addr = Address::from_pubkey_bytes(pubkey_bytes).unwrap();

        assert_eq!(addr.as_hex(), expect_addr);
    }

    #[test]
    fn test_address() {
        let add_str = "CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let bytes = Bytes::from(hex::decode(add_str).unwrap());

        let address = Address::from_bytes(bytes).unwrap();
        assert_eq!(add_str, address.as_hex().to_uppercase());
    }
}
