use std::collections::BTreeMap;
use std::fmt;

use bytes::Bytes;
use hasher::{Hasher, HasherKeccak};
use lazy_static::lazy_static;
use muta_codec_derive::RlpFixedCodec;
use serde::de;
use serde::{Deserialize, Serialize};

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::traits::SchemaGenerator;
use crate::types::TypesError;
use crate::ProtocolResult;

pub const METADATA_KEY: &str = "metadata";

lazy_static! {
    static ref HASHER_INST: HasherKeccak = HasherKeccak::new();
}

/// The height of the genesis block.
pub const GENESIS_HEIGHT: u64 = 0;

/// Hash length
const HASH_LEN: usize = 32;

// Should started with 0x
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Hex(String);

impl Hex {
    pub fn from_string(s: String) -> ProtocolResult<Self> {
        if s.starts_with("0x") {
            Ok(Self(s))
        } else {
            Err(TypesError::HexPrefix.into())
        }
    }

    pub fn as_string(&self) -> String {
        self.0.to_owned()
    }

    pub fn as_string_trim0x(&self) -> String {
        (&self.0[2..]).to_owned()
    }
}

impl Serialize for Hex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

struct HexVisitor;

impl<'de> de::Visitor<'de> for HexVisitor {
    type Value = Hex;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expect a hex string")
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Hex::from_string(v).map_err(|e| de::Error::custom(e.to_string()))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Hex::from_string(v.to_owned()).map_err(|e| de::Error::custom(e.to_string()))
    }
}

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_string(HexVisitor)
    }
}

#[derive(RlpFixedCodec, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash(Bytes);
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
        let out = HASHER_INST.digest(&bytes);
        Self(Bytes::from(out))
    }

    pub fn from_empty() -> Self {
        let out = HASHER_INST.digest(&rlp::NULL_RLP);
        Self(Bytes::from(out))
    }

    /// Converts the byte array to a Hash type.
    /// Note: if you want to compute the hash value of the byte array, you
    /// should call `fn digest`.
    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        ensure_len(bytes.len(), HASH_LEN)?;

        Ok(Self(bytes))
    }

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s)?;
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let bytes = Bytes::from(bytes);
        Self::from_bytes(bytes)
    }

    pub fn as_bytes(&self) -> Bytes {
        self.0.clone()
    }

    pub fn as_hex(&self) -> String {
        "0x".to_owned() + &hex::encode(self.0.clone())
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

#[derive(RlpFixedCodec, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Address(Bytes);

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

        Ok(Self(bytes))
    }

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s)?;
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let bytes = Bytes::from(bytes);
        Self::from_bytes(bytes)
    }

    pub fn as_bytes(&self) -> Bytes {
        self.0.clone()
    }

    pub fn as_hex(&self) -> String {
        "0x".to_owned() + &hex::encode(self.0.clone())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_hex())
    }
}

#[derive(RlpFixedCodec, Deserialize, Default, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub chain_id:        Hash,
    pub common_ref:      Hex,
    pub timeout_gap:     u64,
    pub cycles_limit:    u64,
    pub cycles_price:    u64,
    pub interval:        u64,
    pub verifier_list:   Vec<ValidatorExtend>,
    pub propose_ratio:   u64,
    pub prevote_ratio:   u64,
    pub precommit_ratio: u64,
    pub brake_ratio:     u64,
    pub tx_num_limit:    u64,
    pub max_tx_size:     u64,
}

#[derive(RlpFixedCodec, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct ValidatorExtend {
    pub bls_pub_key:    Hex,
    pub address:        Address,
    pub propose_weight: u32,
    pub vote_weight:    u32,
}

impl SchemaGenerator for Metadata {
    fn name() -> String {
        "Metadata".to_owned()
    }

    fn schema(register: &mut BTreeMap<String, String>) {
        let meta_schema = r#"type Metadata {
  chain_id: Hash!
  common_ref: Hex!
  timeout_gap: U64!
  cycles_limit: U64!
  cycles_price: U64!
  interval: U64!
  verifier_list: [ValidatorExtend!]!
  prevote_ratio: U64!
  precommit_ratio: U64!
  propose_ratio: U64!
  brake_ratio: U64!
  tx_num_limit: U64!
  max_tx_size: U64!
}"#;

        let ve_schema = r#"type ValidatorExtend {
  bls_pub_key: Hex!
  address: Address!
  propose_weight: U32!
  vote_weight: U32!
}"#;
        register.insert("Metadata".to_owned(), meta_schema.to_owned());
        register.insert("ValidatorExtend".to_owned(), ve_schema.to_owned());
        u32::schema(register);
        u64::schema(register);
        Hash::schema(register);
        Address::schema(register);
        Hex::schema(register);
    }
}

impl fmt::Debug for ValidatorExtend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bls_pub_key = self.bls_pub_key.as_string_trim0x();
        let pk = if bls_pub_key.len() > 8 {
            unsafe { bls_pub_key.get_unchecked(0..8) }
        } else {
            bls_pub_key.as_str()
        };

        write!(
            f,
            "bls public key {:?}, address {:?}, propose weight {}, vote weight {}",
            pk,
            self.address.as_hex(),
            self.propose_weight,
            self.vote_weight
        )
    }
}

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct ChainSchema {
    pub schema: Vec<ServiceSchema>,
}

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct ServiceSchema {
    pub service: String,
    pub method:  String,
    pub event:   String,
}

fn clean_0x(s: &str) -> ProtocolResult<&str> {
    if s.starts_with("0x") || s.starts_with("0X") {
        Ok(&s[2..])
    } else {
        Err(TypesError::HexPrefix.into())
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
        let expect_addr = "0xc38f8210896e11a75e1a1f13805d39088d157d7f";

        let pubkey_bytes = Bytes::from(hex::decode(pubkey).unwrap());
        let addr = Address::from_pubkey_bytes(pubkey_bytes).unwrap();

        assert_eq!(addr.as_hex(), expect_addr);
    }

    #[test]
    fn test_address() {
        let add_str = "CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let bytes = Bytes::from(hex::decode(add_str).unwrap());

        let address = Address::from_bytes(bytes).unwrap();
        assert_eq!(add_str, &address.as_hex().to_uppercase().as_str()[2..]);
    }
}
