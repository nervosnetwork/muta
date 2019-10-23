#![allow(clippy::all)]
use std::collections::BTreeMap;
use std::fmt;

use bytes::Bytes;
use hasher::{Hasher, HasherKeccak};
use lazy_static::lazy_static;
use num_bigint::BigUint;

use crate::types::TypesError;
use crate::ProtocolResult;

lazy_static! {
    static ref HASHER_INST: HasherKeccak = HasherKeccak::new();
}

/// The epoch ID of the genesis epoch.
pub const GENESIS_EPOCH_ID: u64 = 0;

/// Hash length
const HASH_LEN: usize = 32;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
        Bytes::from(self.0.as_ref())
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
const ADDRESS_LEN: usize = 21;
/// Magic number of account address.
const ACCOUNT_ADDRESS_MAGIC: u8 = 0x10;
/// Magic number of asset contract address.
const ASSET_CONTRACT_ADDRESS_MAGIC: u8 = 0x20;
/// Magic number of app contract address.
const APP_CONTRACT_ADDRESS_MAGIC: u8 = 0x21;
/// Magic number of library contract address.
const LIBRARY_CONTRACT_ADDRESS_MAGIC: u8 = 0x22;
/// Magic number of native contract address.
const NATIVE_CONTRACT_ADDRESS_MAGIC: u8 = 0x23;

/// Contract type
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContractType {
    // Asset contract
    Asset,
    // App contract, the code in the contract is allowed to change the state world.
    App,
    // Library contract, the code in the contract is not allowed to change the state world.
    Library,
    // Native contract.
    Native,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Address {
    User(UserAddress),
    Contract(ContractAddress),
}

impl Address {
    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        if let Some(flag) = bytes.get(0) {
            match *flag {
                ACCOUNT_ADDRESS_MAGIC => Ok(Address::User(UserAddress::from_bytes(bytes)?)),
                _ => Ok(Address::Contract(ContractAddress::from_bytes(bytes)?)),
            }
        } else {
            Err(TypesError::InvalidAddress {
                address: hex::encode(bytes.to_vec()),
            }
            .into())
        }
    }

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s);
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let bytes = Bytes::from(bytes);
        Self::from_bytes(bytes)
    }

    pub fn as_bytes(&self) -> Bytes {
        match self {
            Address::User(user) => user.as_bytes(),
            Address::Contract(contract) => contract.as_bytes(),
        }
    }

    pub fn as_hex(&self) -> String {
        match self {
            Address::User(user) => user.as_hex(),
            Address::Contract(contract) => contract.as_hex(),
        }
    }
}

/// The address consists of 21 bytes, the first of which is a magic number that
/// identifies which type the address belongs to.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct InnerAddress([u8; ADDRESS_LEN]);

/// Note: the account address here is an external account, not a contract.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UserAddress {
    inner: InnerAddress,
}

/// Contract address.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContractAddress {
    inner:         InnerAddress,
    contract_type: ContractType,
}

impl InnerAddress {
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

impl fmt::Debug for InnerAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0.to_vec()))
    }
}

impl UserAddress {
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

        let inner = InnerAddress::from_bytes(bytes)?;
        Ok(UserAddress { inner })
    }

    pub fn from_pubkey_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        let mut hash_val = Hash::digest(bytes).as_hex();

        hash_val.truncate(40);
        hash_val.insert_str(0, &hex::encode([ACCOUNT_ADDRESS_MAGIC]));

        let decoded =
            hex::decode(hash_val.clone()).map_err(|error| TypesError::FromHex { error })?;

        Self::from_bytes(decoded.into())
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
            NATIVE_CONTRACT_ADDRESS_MAGIC => ContractType::Native,
            _ => {
                return Err(TypesError::InvalidAddress {
                    address: hex::encode(bytes.to_vec()),
                }
                .into())
            }
        };

        let inner = InnerAddress::from_bytes(bytes)?;
        Ok(ContractAddress {
            inner,
            contract_type,
        })
    }

    pub fn from_code(
        code: Bytes,
        nonce: u64,
        contract_type: ContractType,
    ) -> ProtocolResult<ContractAddress> {
        let nonce_bytes: [u8; 8] = nonce.to_be_bytes();
        let mut hash_bytes = Hash::digest(Bytes::from(
            [code, Bytes::from(nonce_bytes.to_vec())].concat(),
        ))
        .as_bytes();
        hash_bytes.truncate(20);

        match contract_type {
            ContractType::Asset => Self::from_bytes(Bytes::from(
                [
                    Bytes::from([ASSET_CONTRACT_ADDRESS_MAGIC].to_vec()),
                    hash_bytes,
                ]
                .concat(),
            )),
            ContractType::App => Self::from_bytes(Bytes::from(
                [
                    Bytes::from([APP_CONTRACT_ADDRESS_MAGIC].to_vec()),
                    hash_bytes,
                ]
                .concat(),
            )),
            ContractType::Library => Self::from_bytes(Bytes::from(
                [
                    Bytes::from([LIBRARY_CONTRACT_ADDRESS_MAGIC].to_vec()),
                    hash_bytes,
                ]
                .concat(),
            )),
            ContractType::Native => Self::from_bytes(Bytes::from(
                [
                    Bytes::from([NATIVE_CONTRACT_ADDRESS_MAGIC].to_vec()),
                    hash_bytes,
                ]
                .concat(),
            )),
        }
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

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub id:              AssetID,
    pub name:            String,
    pub symbol:          String,
    pub supply:          Balance,
    pub manage_contract: ContractAddress,
    pub storage_root:    MerkleRoot,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Fee {
    pub asset_id: AssetID,
    pub cycle:    u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Account {
    User(UserAccount),
    Contract(ContractAccount),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserAccount {
    pub nonce:  u64,
    pub assets: BTreeMap<AssetID, AssetInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetInfo {
    pub balance:  Balance,
    pub approved: BTreeMap<ContractAddress, ApprovedInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovedInfo {
    pub max:  Balance,
    pub used: Balance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractAccount {
    pub nonce:        u64,
    pub assets:       BTreeMap<AssetID, Balance>,
    pub storage_root: MerkleRoot,
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

    use super::{ContractAddress, Hash, UserAddress};

    #[test]
    fn test_hash() {
        let hash = Hash::digest(Bytes::from("xxxxxx"));

        let bytes = hash.as_bytes();
        Hash::from_bytes(bytes).unwrap();
    }

    #[test]
    fn test_from_pubkey_bytes() {
        let pubkey = "031313016e9670deb49779c1b0c646d6a25a545712658f9781995f623bcd0d0b3d";
        let expect_addr = "10c38f8210896e11a75e1a1f13805d39088d157d7f";

        let pubkey_bytes = Bytes::from(hex::decode(pubkey).unwrap());
        let addr = UserAddress::from_pubkey_bytes(pubkey_bytes).unwrap();

        assert_eq!(addr.as_hex(), expect_addr);
    }

    #[test]
    fn test_address() {
        // account address
        let add_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let bytes = Bytes::from(hex::decode(add_str).unwrap());

        let address = UserAddress::from_bytes(bytes).unwrap();
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
