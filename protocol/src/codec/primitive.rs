use std::{convert::TryFrom, default::Default};

use bytes::Bytes;
use derive_more::From;
use prost::{Enumeration, Message};

use crate::{
    codec::{CodecError, ProtocolCodecSync},
    field, impl_default_bytes_codec_for,
    types::primitive as protocol_primitive,
    ProtocolError, ProtocolResult,
};

// #####################
// Protobuf
// #####################

// TODO: change to Amount?
#[derive(Clone, Message)]
pub struct Balance {
    #[prost(bytes, tag = "1")]
    pub value: Vec<u8>,
}

#[derive(Clone, Message, From)]
pub struct Hash {
    #[prost(bytes, tag = "1")]
    pub value: Vec<u8>,
}

#[derive(Clone, Message, From)]
pub struct MerkleRoot {
    #[prost(message, tag = "1")]
    pub value: Option<Hash>,
}

#[derive(Clone, Message, From)]
pub struct AssetID {
    #[prost(message, tag = "1")]
    pub value: Option<Hash>,
}

#[derive(Clone, Message, From)]
pub struct Address {
    #[prost(bytes, tag = "1")]
    pub value: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct AccountAddress {
    #[prost(message, tag = "1")]
    pub value: Option<Address>,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Enumeration)]
pub enum ContractType {
    Asset = 0,
    Library = 1,
    App = 2,
}

#[derive(Clone, Message)]
pub struct ContractAddress {
    #[prost(message, tag = "1")]
    pub value: Option<Address>,

    #[prost(enumeration = "ContractType", tag = "2")]
    pub contract_type: i32,
}

#[derive(Clone, Message)]
pub struct Asset {
    #[prost(message, tag = "1")]
    pub id: Option<AssetID>,

    #[prost(string, tag = "2")]
    pub name: String,

    #[prost(string, tag = "3")]
    pub symbol: String,

    #[prost(message, tag = "4")]
    pub supply: Option<Balance>,

    #[prost(message, tag = "5")]
    pub manage_contract: Option<ContractAddress>,

    #[prost(message, tag = "6")]
    pub storage_root: Option<MerkleRoot>,
}

#[derive(Clone, Message)]
pub struct Fee {
    #[prost(message, tag = "1")]
    pub asset_id: Option<AssetID>,

    #[prost(uint64, tag = "2")]
    pub cycle: u64,
}

// #####################
// Conversion
// #####################

// Balance

impl From<protocol_primitive::Balance> for Balance {
    fn from(balance: protocol_primitive::Balance) -> Balance {
        let mut bytes = [0u8; 32];
        balance.to_big_endian(&mut bytes);

        let value = Bytes::from(bytes.as_ref()).to_vec();
        Balance { value }
    }
}

impl TryFrom<Balance> for protocol_primitive::Balance {
    type Error = ProtocolError;

    fn try_from(ser_balance: Balance) -> Result<protocol_primitive::Balance, Self::Error> {
        let bytes: Bytes = Bytes::from(ser_balance.value);
        ensure_len(bytes.len(), 8 * 4)?;

        Ok(protocol_primitive::Balance::from_big_endian(&bytes))
    }
}

// Hash

impl From<protocol_primitive::Hash> for Hash {
    fn from(hash: protocol_primitive::Hash) -> Hash {
        let value = hash.as_bytes().to_vec();

        Hash { value }
    }
}

impl TryFrom<Hash> for protocol_primitive::Hash {
    type Error = ProtocolError;

    fn try_from(hash: Hash) -> Result<protocol_primitive::Hash, Self::Error> {
        let bytes = Bytes::from(hash.value);

        protocol_primitive::Hash::from_bytes(bytes)
    }
}

// MerkleRoot

impl From<protocol_primitive::MerkleRoot> for MerkleRoot {
    fn from(root: protocol_primitive::MerkleRoot) -> MerkleRoot {
        let value = Some(Hash::from(root));

        MerkleRoot { value }
    }
}

impl TryFrom<MerkleRoot> for protocol_primitive::MerkleRoot {
    type Error = ProtocolError;

    fn try_from(root: MerkleRoot) -> Result<protocol_primitive::MerkleRoot, Self::Error> {
        let hash = field!(root.value, "MerkleRoot", "value")?;

        protocol_primitive::Hash::try_from(hash)
    }
}

// AssetID

impl From<protocol_primitive::AssetID> for AssetID {
    fn from(id: protocol_primitive::AssetID) -> AssetID {
        let value = Some(Hash::from(id));

        AssetID { value }
    }
}

impl TryFrom<AssetID> for protocol_primitive::AssetID {
    type Error = ProtocolError;

    fn try_from(id: AssetID) -> Result<protocol_primitive::AssetID, Self::Error> {
        let hash = field!(id.value, "MerkleRoot", "value")?;

        protocol_primitive::Hash::try_from(hash)
    }
}

// AccountAddress

impl From<protocol_primitive::AccountAddress> for AccountAddress {
    fn from(account: protocol_primitive::AccountAddress) -> AccountAddress {
        let value = account.as_bytes().to_vec();
        let address = Address { value };

        AccountAddress {
            value: Some(address),
        }
    }
}

impl TryFrom<AccountAddress> for protocol_primitive::AccountAddress {
    type Error = ProtocolError;

    fn try_from(
        account: AccountAddress,
    ) -> Result<protocol_primitive::AccountAddress, Self::Error> {
        let address = field!(account.value, "AccountAddress", "value")?;

        let bytes = Bytes::from(address.value);
        protocol_primitive::AccountAddress::from_bytes(bytes)
    }
}

// ContractAddress

impl From<protocol_primitive::ContractAddress> for ContractAddress {
    fn from(contract: protocol_primitive::ContractAddress) -> ContractAddress {
        let value = contract.as_bytes().to_vec();
        let address = Some(Address { value });

        let contract_type = match contract.contract_type() {
            protocol_primitive::ContractType::Asset => ContractType::Asset,
            protocol_primitive::ContractType::Library => ContractType::Library,
            protocol_primitive::ContractType::App => ContractType::App,
        };

        ContractAddress {
            value:         address,
            contract_type: contract_type as i32,
        }
    }
}

impl TryFrom<ContractAddress> for protocol_primitive::ContractAddress {
    type Error = ProtocolError;

    fn try_from(
        contract: ContractAddress,
    ) -> Result<protocol_primitive::ContractAddress, Self::Error> {
        let address = field!(contract.value, "ContractAddress", "value")?;

        let bytes = Bytes::from(address.value);
        protocol_primitive::ContractAddress::from_bytes(bytes)
    }
}

// Asset

impl From<protocol_primitive::Asset> for Asset {
    fn from(asset: protocol_primitive::Asset) -> Asset {
        let id = AssetID::from(asset.id);
        let supply = Balance::from(asset.supply);
        let manage_contract = ContractAddress::from(asset.manage_contract);
        let storage_root = MerkleRoot::from(asset.storage_root);

        Asset {
            id:              Some(id),
            name:            asset.name,
            symbol:          asset.symbol,
            supply:          Some(supply),
            manage_contract: Some(manage_contract),
            storage_root:    Some(storage_root),
        }
    }
}

impl TryFrom<Asset> for protocol_primitive::Asset {
    type Error = ProtocolError;

    fn try_from(asset: Asset) -> Result<protocol_primitive::Asset, Self::Error> {
        let id = field!(asset.id, "Asset", "id")?;
        let supply = field!(asset.supply, "Asset", "supply")?;
        let manage_contract = field!(asset.manage_contract, "Asset", "manage_contract")?;
        let storage_root = field!(asset.storage_root, "Asset", "storage_root")?;

        let asset = protocol_primitive::Asset {
            id:              protocol_primitive::AssetID::try_from(id)?,
            name:            asset.name,
            symbol:          asset.symbol,
            supply:          protocol_primitive::Balance::try_from(supply)?,
            manage_contract: protocol_primitive::ContractAddress::try_from(manage_contract)?,
            storage_root:    protocol_primitive::MerkleRoot::try_from(storage_root)?,
        };

        Ok(asset)
    }
}

// Fee

impl From<protocol_primitive::Fee> for Fee {
    fn from(fee: protocol_primitive::Fee) -> Fee {
        let id_hash = Hash::from(fee.asset_id);
        let asset_id = AssetID {
            value: Some(id_hash),
        };

        Fee {
            asset_id: Some(asset_id),
            cycle:    fee.cycle,
        }
    }
}

impl TryFrom<Fee> for protocol_primitive::Fee {
    type Error = ProtocolError;

    fn try_from(fee: Fee) -> Result<protocol_primitive::Fee, Self::Error> {
        let asset_id = field!(fee.asset_id, "Fee", "asset_id")?;
        let id_hash = field!(asset_id.value, "Fee", "asset_id")?;

        let fee = protocol_primitive::Fee {
            asset_id: protocol_primitive::Hash::try_from(id_hash)?,
            cycle:    fee.cycle,
        };

        Ok(fee)
    }
}

// #####################
// Codec
// #####################

// MerkleRoot and AssetID are just Hash aliases
impl_default_bytes_codec_for!(primitive, [
    Balance,
    Hash,
    AccountAddress,
    ContractAddress,
    Asset,
    Fee
]);

// #####################
// Util
// #####################

fn ensure_len(real: usize, expect: usize) -> Result<(), CodecError> {
    if real != expect {
        return Err(CodecError::WrongBytesLength { expect, real });
    }

    Ok(())
}
