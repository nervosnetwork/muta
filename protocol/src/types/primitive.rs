use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

use bech32::{self, FromBase32, ToBase32};
use bytes::Bytes;
use hasher::{Hasher, HasherKeccak};
use lazy_static::lazy_static;
use muta_codec_derive::RlpFixedCodec;
use ophelia::UncompressedPublicKey;
use ophelia_secp256k1::Secp256k1PublicKey;
use serde::de;
use serde::{Deserialize, Serialize};

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::TypesError;
use crate::ProtocolResult;

pub const METADATA_KEY: &str = "metadata";

lazy_static! {
    static ref HASHER_INST: HasherKeccak = HasherKeccak::new();
}

/// The address bech32 hrp
static ADDRESS_HRP: &str = include!(concat!(env!("OUT_DIR"), "/address_hrp.rs"));

/// The height of the genesis block.
pub const GENESIS_HEIGHT: u64 = 0;

/// Hash length
const HASH_LEN: usize = 32;

// Should started with 0x
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hex(String);

impl Hex {
    pub fn from_string(s: String) -> ProtocolResult<Self> {
        if (!s.starts_with("0x") && !s.starts_with("0X")) || s.len() < 3 {
            return Err(TypesError::HexPrefix.into());
        }

        hex::decode(&s[2..]).map_err(|error| TypesError::FromHex { error })?;
        Ok(Hex(s))
    }

    pub fn as_string(&self) -> String {
        self.0.to_owned()
    }

    pub fn as_string_trim0x(&self) -> String {
        (&self.0[2..]).to_owned()
    }

    pub fn decode(&self) -> Bytes {
        Bytes::from(hex::decode(&self.0[2..]).expect("impossible, already checked in from_string"))
    }
}

impl Default for Hex {
    fn default() -> Self {
        Hex::from_string("0x1".to_owned()).expect("Hex must start with 0x")
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
        serializer.serialize_str(&self.to_string())
    }
}

struct AddressVisitor;

impl<'de> de::Visitor<'de> for AddressVisitor {
    type Value = Address;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expect a bech32 string")
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Address::from_str(&v).map_err(|e| de::Error::custom(e.to_string()))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Address::from_str(&v).map_err(|e| de::Error::custom(e.to_string()))
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
    pub fn from_pubkey_bytes(mut bytes: Bytes) -> ProtocolResult<Self> {
        let uncompressed_pubkey_len = <Secp256k1PublicKey as UncompressedPublicKey>::LENGTH;
        if bytes.len() != uncompressed_pubkey_len {
            let pubkey = Secp256k1PublicKey::try_from(bytes.as_ref())
                .map_err(|_| TypesError::InvalidPublicKey)?;
            bytes = pubkey.to_uncompressed_bytes();
        }
        let bytes = bytes.split_off(1); // Drop first byte

        let hash = Hash::digest(bytes);
        Self::from_hash(hash)
    }

    pub fn from_hash(hash: Hash) -> ProtocolResult<Self> {
        let hash_val = hash.as_bytes();
        ensure_len(hash_val.len(), HASH_LEN)?;

        Self::from_bytes(Bytes::copy_from_slice(&hash_val[12..]))
    }

    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        ensure_len(bytes.len(), ADDRESS_LEN)?;

        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> Bytes {
        self.0.clone()
    }
}

impl FromStr for Address {
    type Err = TypesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, data) = bech32::decode(s).map_err(TypesError::from)?;
        let bytes = Vec::<u8>::from_base32(&data).map_err(TypesError::from)?;

        Ok(Address(Bytes::from(bytes)))
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // NOTE: ADDRESS_HRP was verified in protocol/build.rs
        bech32::encode_to_fmt(f, ADDRESS_HRP, &self.0.to_base32()).unwrap()
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // NOTE: ADDRESS_HRP was verified in protocol/build.rs
        bech32::encode_to_fmt(f, ADDRESS_HRP, &self.0.to_base32()).unwrap()
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
    pub pub_key:        Hex,
    pub address:        Address,
    pub propose_weight: u32,
    pub vote_weight:    u32,
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
            "bls public key {:?}, public key {:?}, address {:?} propose weight {}, vote weight {}",
            pk, self.pub_key, self.address, self.propose_weight, self.vote_weight
        )
    }
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
    use bech32::{self, FromBase32};
    use bytes::Bytes;

    use super::{Address, Hash, ValidatorExtend};
    use crate::{fixed_codec::FixedCodec, types::Hex};

    #[test]
    fn test_hash() {
        let hash = Hash::digest(Bytes::from("xxxxxx"));

        let bytes = hash.as_bytes();
        Hash::from_bytes(bytes).unwrap();
    }

    #[test]
    fn test_from_pubkey_bytes() {
        let pubkey = "02ee34d1ce8270cd236e9455d4ab9e756c4478779b1a20d7ce1c247af61ec2be3b";
        let expect_addr = "muta1mu4rq2mwvy2h4uss4al7u7ejj5rlcdmpeurh24";

        let pubkey_bytes = Bytes::from(hex::decode(pubkey).unwrap());
        let addr = Address::from_pubkey_bytes(pubkey_bytes).unwrap();

        assert_eq!(addr.to_string(), expect_addr);
    }

    #[test]
    fn test_address() {
        let add_str = "muta1mu4rq2mwvy2h4uss4al7u7ejj5rlcdmpeurh24";
        let (_, data) = bech32::decode(add_str).unwrap();
        let bytes = Bytes::from(Vec::<u8>::from_base32(&data).unwrap());

        let address = Address::from_bytes(bytes).unwrap();
        assert_eq!(add_str, &address.to_string());
    }

    #[test]
    fn test_hex() {
        let hex_str = "0x112233445566AABBcc";
        let hex = Hex::from_string(hex_str.to_owned()).unwrap();

        assert_eq!(hex_str, hex.0.as_str());
    }

    #[test]
    fn test_validator_extend() {
        let extend = ValidatorExtend {
            bls_pub_key: Hex::from_string("0x040c49fc3191406e86defff7c4d5f5a177acc758f24aaf5b820ff298260c6a994b7df3c2b5cd472466641db41f43f02f8109199b2fade972a85a6086a3b264280f034f3e307219950259d195de2f33e132c4e9cb8b5e9cc33f5b649a63e0a4dcba".to_owned()).unwrap(),
            pub_key: Hex::from_string("0x02ee34d1ce8270cd236e9455d4ab9e756c4478779b1a20d7ce1c247af61ec2be3b".to_owned()).unwrap(),     address: "muta1mu4rq2mwvy2h4uss4al7u7ejj5rlcdmpeurh24".parse().unwrap(),
            propose_weight: 1,
            vote_weight:    1,
        };

        let decoded = ValidatorExtend::decode_fixed(extend.encode_fixed().unwrap()).unwrap();
        assert_eq!(decoded, extend);
    }
}
