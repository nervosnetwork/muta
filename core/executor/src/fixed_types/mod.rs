use bytes::Bytes;

use protocol::fixed_codec::ProtocolFixedCodec;
use protocol::traits::executor::{ContractSchema, ContractSer};
use protocol::types::{Account, Address, Asset, AssetID};
use protocol::ProtocolResult;

pub struct FixedAssetSchema;
impl ContractSchema for FixedAssetSchema {
    type Key = FixedAssetID;
    type Value = FixedAsset;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedAssetID {
    inner: AssetID,
}

impl FixedAssetID {
    pub fn new(inner: AssetID) -> Self {
        Self { inner }
    }
}

impl ContractSer for FixedAssetID {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(self.inner.as_bytes())
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        let id = AssetID::from_bytes(bytes)?;
        Ok(FixedAssetID { inner: id })
    }
}

/// the `FixedAsset` is a wrapper type of asset just to provide a consistent
/// serialization algorithm `rlp`.
#[derive(Clone, Debug)]
pub struct FixedAsset {
    pub inner: Asset,
}

impl FixedAsset {
    pub fn new(inner: Asset) -> Self {
        Self { inner }
    }
}

impl ContractSer for FixedAsset {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(self.inner.encode_fixed()?)
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(Self {
            inner: Asset::decode_fixed(bytes)?,
        })
    }
}

pub struct FixedAccountSchema;
impl ContractSchema for FixedAccountSchema {
    type Key = FixedAddress;
    type Value = FixedAccount;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedAddress {
    inner: Address,
}

impl FixedAddress {
    pub fn new(inner: Address) -> Self {
        Self { inner }
    }
}

impl ContractSer for FixedAddress {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(self.inner.encode_fixed()?)
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        let address = Address::decode_fixed(bytes)?;
        Ok(FixedAddress { inner: address })
    }
}

/// the `FixedAccount` is a wrapper type of asset just to provide a consistent
/// serialization algorithm `rlp`.
#[derive(Clone, Debug)]
pub struct FixedAccount {
    pub inner: Account,
}

impl FixedAccount {
    pub fn new(inner: Account) -> Self {
        Self { inner }
    }
}

impl ContractSer for FixedAccount {
    fn encode(&self) -> ProtocolResult<Bytes> {
        Ok(self.inner.encode_fixed()?)
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(Self {
            inner: Account::decode_fixed(bytes)?,
        })
    }
}
