#![allow(clippy::all)]
use std::fmt;

use bytes::Bytes;
use num_bigint::BigUint;
use sha3::{Digest, Sha3_256};

use crate::types::TypesError;
use crate::ProtocolResult;

/// Hash length
const HASH_LEN: usize = 32;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Hash([u8; HASH_LEN]);
/// Balance
pub type Balance = BigUint;
/// Merkel root hash
pub type MerkleRoot = Hash;
/// Asset ID
pub type AssetID = Hash;

impl Hash {
    /// Enter an array of bytes to get a 32-bit hash.
    /// Note: sha3 is used for the time being and may be replaced with other
    /// hashing algorithms later.
    pub fn digest(bytes: Bytes) -> Self {
        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&Sha3_256::digest(&bytes));

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
        Bytes::from(self.0.as_ref())
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_hex())
    }
}

/// Address length.
const ADDRESS_LEN: usize = 21;
/// Magic number of account address.
const ACCOUNT_ADDRESS_MAGIC: u8 = 0x10;
/// Magic number of asset contract address.
const ASSET_CONTRACT_ADDRESS_MAGIC: u8 = 0x20;
/// Magic number of app contract address.
const APP_CONTRACT_ADDRESS_MAGIC: u8 = 0x21;
/// Magic number of library contract address.
const LIBRARY_CONTRACT_ADDRESS_MAGIC: u8 = 0x22;

/// Contract type
#[derive(Clone, Debug)]
pub enum ContractType {
    // Asset contract
    Asset,
    // App contract, the code in the contract is allowed to change the state world.
    App,
    // Library contract, the code in the contract is not allowed to change the state world.
    Library,
}

/// The address consists of 21 bytes, the first of which is a magic number that
/// identifies which type the address belongs to.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Address([u8; ADDRESS_LEN]);

/// Note: the account address here is an external account, not a contract.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AccountAddress {
    inner: Address,
}

/// Contract address.
#[derive(Clone, Debug)]
pub struct ContractAddress {
    inner:         Address,
    contract_type: ContractType,
}

impl Address {
    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        ensure_len(bytes.len(), ADDRESS_LEN)?;

        let mut out = [0u8; ADDRESS_LEN];
        out.copy_from_slice(&bytes);
        Ok(Self(out))
    }

    pub fn as_bytes(&self) -> Bytes {
        Bytes::from(self.0.as_ref())
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0.to_vec()))
    }
}

impl AccountAddress {
    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        let magic = bytes.get(0).ok_or_else(|| TypesError::InvalidAddress {
            address: hex::encode(bytes.to_vec()),
        })?;

        if *magic != ACCOUNT_ADDRESS_MAGIC {
            return Err(TypesError::InvalidAddress {
                address: hex::encode(bytes.to_vec()),
            }
            .into());
        }

        let inner = Address::from_bytes(bytes)?;
        Ok(AccountAddress { inner })
    }

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s);
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let bytes = Bytes::from(bytes);
        Self::from_bytes(bytes)
    }

    pub fn as_hex(&self) -> String {
        self.inner.as_hex()
    }

    pub fn as_bytes(&self) -> Bytes {
        self.inner.as_bytes()
    }
}

impl ContractAddress {
    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        let magic = bytes.get(0).ok_or_else(|| TypesError::InvalidAddress {
            address: hex::encode(bytes.to_vec()),
        })?;

        let contract_type = match *magic {
            ASSET_CONTRACT_ADDRESS_MAGIC => ContractType::Asset,
            APP_CONTRACT_ADDRESS_MAGIC => ContractType::App,
            LIBRARY_CONTRACT_ADDRESS_MAGIC => ContractType::Library,
            _ => {
                return Err(TypesError::InvalidAddress {
                    address: hex::encode(bytes.to_vec()),
                }
                .into())
            }
        };

        let inner = Address::from_bytes(bytes)?;
        Ok(ContractAddress {
            inner,
            contract_type,
        })
    }

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s);
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let bytes = Bytes::from(bytes);
        Self::from_bytes(bytes)
    }

    pub fn as_hex(&self) -> String {
        self.inner.as_hex()
    }

    pub fn as_bytes(&self) -> Bytes {
        self.inner.as_bytes()
    }

    pub fn contract_type(&self) -> ContractType {
        self.contract_type.clone()
    }
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub id:              AssetID,
    pub name:            String,
    pub symbol:          String,
    pub supply:          Balance,
    pub manage_contract: ContractAddress,
    pub storage_root:    MerkleRoot,
}

#[derive(Clone, Debug)]
pub struct Fee {
    pub asset_id: AssetID,
    pub cycle:    u64,
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

    use super::{AccountAddress, ContractAddress, Hash};

    #[test]
    fn test_hash() {
        let hash = Hash::digest(Bytes::from("xxxxxx"));

        let bytes = hash.as_bytes();
        Hash::from_bytes(bytes).unwrap();
    }

    #[test]
    fn test_address() {
        // account address
        let add_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let bytes = Bytes::from(hex::decode(add_str).unwrap());

        let address = AccountAddress::from_bytes(bytes).unwrap();
        assert_eq!(add_str, address.as_hex().to_uppercase());

        // asset contract  address
        let add_str = "20CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let bytes = Bytes::from(hex::decode(add_str).unwrap());

        let address = ContractAddress::from_bytes(bytes).unwrap();
        assert_eq!(add_str, address.as_hex().to_uppercase());

        // app contract  address
        let add_str = "21CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let bytes = Bytes::from(hex::decode(add_str).unwrap());

        let address = ContractAddress::from_bytes(bytes).unwrap();
        assert_eq!(add_str, address.as_hex().to_uppercase());

        // library contract  address
        let add_str = "22CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let bytes = Bytes::from(hex::decode(add_str).unwrap());

        let address = ContractAddress::from_bytes(bytes).unwrap();
        assert_eq!(add_str, address.as_hex().to_uppercase());
    }
}
