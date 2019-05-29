use std::fmt;

use numext_fixed_hash::H160;
pub use numext_fixed_uint::U256;
use rlp::{Encodable, RlpStream};
use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "hashlib-sha3")]
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

    /// Mixed-case checksum address encoding. Note: without 0x prefix!
    /// See: https://github.com/ethereum/EIPs/blob/master/EIPS/eip-55.md
    pub fn as_checksum_hex(&self) -> String {
        let address = self.as_hex();
        let address_char_vec: Vec<char> = address.chars().collect();
        let hash = Hash::digest(address.as_bytes()).as_hex();
        let hash_char_vec: Vec<char> = hash.chars().collect();
        let mut ret = String::new();

        for i in 0..40 {
            let c = hash_char_vec[i];
            if c as u8 >= 56 {
                ret.push(address_char_vec[i].to_uppercase().next().unwrap());
            } else {
                ret.push(address_char_vec[i]);
            }
        }
        ret
    }

    pub fn from_hex(input: &str) -> Result<Self, TypesError> {
        let input = clean_0x(input);
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
        s.encoder().encode_value(self.as_bytes());
    }
}

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", self.as_hex()))
    }
}

struct AddressVisitor;

impl<'de> Visitor<'de> for AddressVisitor {
    type Value = Address;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an ASCII hex string whose length is 40, with or without prefix '0x'")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: ::serde::de::Error,
    {
        Address::from_hex(v).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(AddressVisitor)
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

        #[cfg(feature = "hashlib-sha3")]
        out.copy_from_slice(&Sha3_256::digest(raw));

        #[cfg(feature = "hashlib-keccak")]
        out.copy_from_slice(&tiny_keccak::keccak256(raw));

        Hash(H256::from(out))
    }

    pub fn from_fixed_bytes(data: [u8; HASH_LEN]) -> Self {
        let hash = H256::from(data);
        Hash(hash)
    }

    pub fn from_hex(input: &str) -> Result<Self, TypesError> {
        let input = clean_0x(input);
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
        s.encoder().encode_value(self.as_bytes());
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", self.as_hex()))
    }
}

struct HashVisitor;

impl<'de> Visitor<'de> for HashVisitor {
    type Value = Hash;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an ASCII hex string whose length is 64, with or without prefix '0x'")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: ::serde::de::Error,
    {
        Hash::from_hex(v).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Hash, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(HashVisitor)
    }
}

pub fn clean_0x(s: &str) -> &str {
    if s.starts_with("0x") {
        &s[2..]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_derive::{Deserialize, Serialize};

    #[test]
    fn test_hash_addr_serde() {
        #[derive(Debug, Deserialize, Serialize, PartialEq)]
        struct Config {
            timestamp: u64,
            hash:      Hash,
            address:   Address,
        }

        let config = Config {
            timestamp: 1,
            hash:      Hash::from_fixed_bytes([1u8; 32]),
            address:   Address::from_fixed_bytes([2u8; 20]),
        };
        let config_str = serde_json::to_string(&config).unwrap();
        assert_eq!(config_str, r#"{"timestamp":1,"hash":"0x0101010101010101010101010101010101010101010101010101010101010101","address":"0x0202020202020202020202020202020202020202"}"#);
        let config2: Config = serde_json::from_str(&config_str).unwrap();
        assert_eq!(config, config2);

        // serde from string directly
        let config_str = r#"{"timestamp":1,"hash":"0x0101010101010101010101010101010101010101010101010101010101010102","address":"0x0202020202020202020202020202020202020201"}"#;
        let config: Result<Config, _> = serde_json::from_str(&config_str);
        assert!(config.is_ok());

        // without 0x prefix is fine
        let config_str = r#"{"timestamp":1,"hash":"0101010101010101010101010101010101010101010101010101010101010102","address":"0202020202020202020202020202020202020201"}"#;
        let config: Result<Config, _> = serde_json::from_str(&config_str);
        assert!(config.is_ok());

        // wrong serde
        let wrong_config_str =
            r#"{"timestamp":1,"hash":1,"address":"0x0202020202020202020202020202020202020201"}"#;
        let wrong_config: Result<Config, _> = serde_json::from_str(&wrong_config_str);
        assert!(wrong_config.is_err());

        let wrong_config_str = r#"{"timestamp":1,"hash":"0x01","address":"0x0202020202020202020202020202020202020201"}"#;
        let wrong_config: Result<Config, _> = serde_json::from_str(&wrong_config_str);
        assert!(wrong_config.is_err());
    }

    #[cfg(feature = "hashlib-keccak")]
    #[test]
    fn test_checksum_encoding() {
        // From: https://github.com/ethereum/EIPs/blob/master/EIPS/eip-55.md#implementation
        let raw = Address::from_hex("0xfb6916095ca1df60bb79ce92ce3ea74c37c5d359").unwrap();
        let ret = "fB6916095ca1df60bB79Ce92cE3Ea74c37c5d359";
        assert_eq!(raw.as_checksum_hex(), ret);

        // From: https://github.com/ethereum/EIPs/blob/master/EIPS/eip-55.md#test-cases
        for (raw, ret) in &[
            (
                "0x52908400098527886E0F7030069857D2E4169EE7",
                "52908400098527886E0F7030069857D2E4169EE7",
            ),
            (
                "0x8617E340B3D01FA5F11F306F4090FD50E238070D",
                "8617E340B3D01FA5F11F306F4090FD50E238070D",
            ),
            (
                "0xde709f2102306220921060314715629080e2fb77",
                "de709f2102306220921060314715629080e2fb77",
            ),
            (
                "0x27b1fdb04752bbc536007a920d24acb045561c26",
                "27b1fdb04752bbc536007a920d24acb045561c26",
            ),
            (
                "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
                "5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            ),
            (
                "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
                "fB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
            ),
            (
                "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
                "dbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
            ),
            (
                "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
                "D1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
            ),
        ] {
            let raw = Address::from_hex(raw).unwrap();
            assert_eq!(raw.as_checksum_hex(), *ret);
        }
    }
}
