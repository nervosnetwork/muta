use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;
use bech32::{self, FromBase32, ToBase32};
use bytes::Bytes;
use hasher::{Hasher, HasherKeccak};
use lazy_static::lazy_static;
use muta_codec_derive::RlpFixedCodec;
use ophelia::UncompressedPublicKey;
use ophelia_secp256k1::Secp256k1PublicKey;
use serde::de;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::TypesError;
use crate::ProtocolResult;

pub const METADATA_KEY: &str = "metadata";

lazy_static! {
    static ref HASHER_INST: HasherKeccak = HasherKeccak::new();
    static ref ADDRESS_HRP: ArcSwap<String> = ArcSwap::from(Arc::new("muta".to_owned()));
    static ref ADDRESS_HRP_INITED: AtomicBool = AtomicBool::new(false);
}

pub fn address_hrp() -> Arc<String> {
    ADDRESS_HRP.load_full()
}

pub fn init_address_hrp(address_hrp: String) {
    if ADDRESS_HRP_INITED.load(Ordering::SeqCst) {
        panic!("address hrp can only be inited once");
    }

    // Verify address hrp
    let hash = HASHER_INST.digest(b"hello muta");
    assert_eq!(hash.len(), 32);

    let bytes = &hash[12..];
    assert_eq!(bytes.len(), 20);

    bech32::encode(&address_hrp, bytes.to_base32()).expect("invalid address hrp");

    // Set address hrp
    ADDRESS_HRP.store(Arc::new(address_hrp));
    ADDRESS_HRP_INITED.store(true, Ordering::SeqCst);
}

pub fn address_hrp_inited() -> bool {
    ADDRESS_HRP_INITED.load(Ordering::SeqCst)
}

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

    /// Used for byzantine test
    pub fn from_invalid_bytes(bytes: Bytes) -> Self {
        Self(bytes)
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

#[derive(RlpFixedCodec, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address(Bytes);

impl Default for Address {
    fn default() -> Self {
        Address::from_hex("0x0000000000000000000000000000000000000000")
            .expect("Address must consist of 20 bytes")
    }
}

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

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s)?;
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let bytes = Bytes::from(bytes);
        Self::from_bytes(bytes)
    }

    /// Used for byzantine test
    pub fn from_invalid_bytes(bytes: Bytes) -> Self {
        Self(bytes)
    }
}

impl FromStr for Address {
    type Err = TypesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (hrp, data) = bech32::decode(s).map_err(TypesError::from)?;
        if &hrp != address_hrp().as_ref() {
            return Err(TypesError::InvalidAddress {
                address: s.to_owned(),
            });
        }

        let bytes = Vec::<u8>::from_base32(&data).map_err(TypesError::from)?;
        Ok(Address(Bytes::from(bytes)))
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // NOTE: ADDRESS_HRP was verified in init_address_hrp fn
        bech32::encode_to_fmt(f, address_hrp().as_ref(), &self.0.to_base32()).unwrap()
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // NOTE: ADDRESS_HRP was verified in init_address_hrp fn
        bech32::encode_to_fmt(f, address_hrp().as_ref(), &self.0.to_base32()).unwrap()
    }
}

#[derive(RlpFixedCodec, Deserialize, Default, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub chain_id:           Hash,
    pub bech32_address_hrp: String,
    pub common_ref:         Hex,
    pub timeout_gap:        u64,
    pub cycles_limit:       u64,
    pub cycles_price:       u64,
    pub interval:           u64,
    pub verifier_list:      Vec<ValidatorExtend>,
    pub propose_ratio:      u64,
    pub prevote_ratio:      u64,
    pub precommit_ratio:    u64,
    pub brake_ratio:        u64,
    pub tx_num_limit:       u64,
    pub max_tx_size:        u64,
}

impl Metadata {
    pub fn get_hrp_from_json(payload: String) -> String {
        let nodes: Value = serde_json::from_str(payload.as_str())
            .expect("metadata's genesis payload is invalid JSON");
        nodes["bech32_address_hrp"]
            .as_str()
            .expect("bech32_address_hrp in genesis payload is not string?")
            .to_string()
    }
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

    use super::{address_hrp, init_address_hrp, Address, Hash, ValidatorExtend};
    use crate::types::Metadata;
    use crate::{fixed_codec::FixedCodec, types::Hex};

    #[test]
    fn test_hash() {
        let hash = Hash::digest(Bytes::from("xxxxxx"));

        let bytes = hash.as_bytes();
        Hash::from_bytes(bytes).unwrap();
    }

    #[test]
    fn test_from_hex() {
        let address_hex = "0x755cdba6ae4f479f7164792b318b2a06c759833b";
        let address_bech32 = "muta1w4wdhf4wfare7uty0y4nrze2qmr4nqem9j7teu";
        let address = Address::from_hex(address_hex).unwrap();
        assert_eq!(address.to_string(), address_bech32);
    }

    #[test]
    fn test_from_pubkey_bytes() {
        let pubkey = "02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60";
        let expect_addr = "muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705";

        let pubkey_bytes = Bytes::from(hex::decode(pubkey).unwrap());
        let addr = Address::from_pubkey_bytes(pubkey_bytes).unwrap();

        assert_eq!(addr.to_string(), expect_addr);
    }

    #[test]
    fn test_address() {
        let add_str = "muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705";
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
           bls_pub_key: Hex::from_string("0x04102947214862a503c73904deb5818298a186d68c7907bb609583192a7de6331493835e5b8281f4d9ee705537c0e765580e06f86ddce5867812fceb42eecefd209f0eddd0389d6b7b0100f00fb119ef9ab23826c6ea09aadcc76fa6cea6a32724".to_owned()).unwrap(),
           pub_key: Hex::from_string("0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60".to_owned()).unwrap(),
           address: "muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705".parse().unwrap(),
           propose_weight: 1,
           vote_weight:    1,
       };

        let decoded = ValidatorExtend::decode_fixed(extend.encode_fixed().unwrap()).unwrap();
        assert_eq!(decoded, extend);
    }

    // Note: All tests run in same process, change ADDRESS_HRP affects other tests
    #[test]
    #[should_panic(expected = "must set hrp before deserialization")]
    fn test_init_address_hrp() {
        assert_eq!(address_hrp().as_ref(), "muta", "default value");

        let metadata_payload = r#"
        {
            "chain_id": "0xb6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036",
            "bech32_address_hrp": "ham",
            "common_ref": "0x6c747758636859487038",
            "timeout_gap": 20,
            "cycles_limit": 4294967295,
            "cycles_price": 1,
            "interval": 3000,
            "verifier_list": [
               {
                   "bls_pub_key": "0x04102947214862a503c73904deb5818298a186d68c7907bb609583192a7de6331493835e5b8281f4d9ee705537c0e765580e06f86ddce5867812fceb42eecefd209f0eddd0389d6b7b0100f00fb119ef9ab23826c6ea09aadcc76fa6cea6a32724",
                   "pub_key": "0x02ef0cb0d7bc6c18b4bea1f5908d9106522b35ab3c399369605d4242525bda7e60",
                   "address": "ham14e0lmgck835vm2dfm0w3ckv6svmez8fdmq5fts",
                   "propose_weight": 1,
                   "vote_weight": 1
               }
            ],
            "propose_ratio": 15,
            "prevote_ratio": 10,
            "precommit_ratio": 10,
            "brake_ratio": 7,
            "tx_num_limit": 20000,
            "max_tx_size": 1024
        }
        "#;

        let hrp = Metadata::get_hrp_from_json(metadata_payload.to_string());

        assert_eq!("ham".to_string(), hrp, "should be same");

        // this should fail because we did not set hrp to ham like
        // init_address_hrp(hrp);
        serde_json::from_str::<Metadata>(metadata_payload)
            .expect("must set hrp before deserialization");
    }

    #[test]
    #[should_panic(expected = "address hrp can only be inited once")]
    fn test_init_address_hrp_twice() {
        init_address_hrp("muta".to_owned());
        init_address_hrp("muta".to_owned());
    }
}
