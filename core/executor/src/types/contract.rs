use std::collections::BTreeMap;
use std::error::Error;
use std::mem;

use bytes::Bytes;
use derive_more::{Display, From};

use protocol::traits::executor::{ContractSchema, ContractSer};
use protocol::types::{
    Account, Address, ApprovedInfo, Asset, AssetID, AssetInfo, Balance, ContractAccount,
    ContractAddress, Hash, MerkleRoot, UserAccount,
};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

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
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedTypesError::from)?)
    }
}

/// FixedAsset encodable to RLP
impl rlp::Encodable for FixedAsset {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let inner = &self.inner;

        s.begin_list(6);
        s.append(&inner.id.as_bytes().to_vec());
        s.append(&inner.manage_contract.as_bytes().to_vec());
        s.append(&inner.name.as_bytes());
        s.append(&inner.storage_root.as_bytes().to_vec());
        s.append(&inner.supply.to_bytes_be());
        s.append(&inner.symbol.as_bytes());
    }
}

/// RLP decodable trait
impl rlp::Decodable for FixedAsset {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 6 {
            return Err(rlp::DecoderError::RlpInvalidLength);
        }

        let mut values = Vec::with_capacity(6);

        for val in r {
            let data = val.data()?;
            values.push(data)
        }

        let id = Hash::from_bytes(Bytes::from(values[0]))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let manage_contract = ContractAddress::from_bytes(Bytes::from(values[1]))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let name = String::from_utf8(values[2].to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let storage_root = Hash::from_bytes(Bytes::from(values[3]))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let supply = Balance::from_bytes_be(values[4]);
        let symbol = String::from_utf8(values[5].to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        let asset = Asset {
            id,
            manage_contract,
            name,
            storage_root,
            supply,
            symbol,
        };

        Ok(FixedAsset { inner: asset })
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
        match &self.inner {
            Address::User(user) => Ok(user.as_bytes()),
            Address::Contract(contract) => Ok(contract.as_bytes()),
        }
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        let address = Address::from_bytes(bytes)?;
        Ok(FixedAddress { inner: address })
    }
}

const USER_ACCOUNT_FLAG: u8 = 0;
const CONTRACT_ACCOUNT_FLAG: u8 = 1;

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
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedTypesError::from)?)
    }
}

/// FixedAsset encodable to RLP
impl rlp::Encodable for FixedAccount {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let inner = &self.inner;

        match inner {
            Account::User(user) => {
                s.begin_list(3);
                s.append(&USER_ACCOUNT_FLAG);
                s.append(&user.nonce.to_be_bytes());

                let mut asset_list = Vec::with_capacity(user.assets.len());

                for (id, asset_info) in user.assets.iter() {
                    let asset_info = FixedUserAssetInfo {
                        id:       id.clone(),
                        balance:  asset_info.balance.clone(),
                        approved: asset_info.approved.clone(),
                    };

                    asset_list.push(asset_info);
                }

                s.append_list(&asset_list);
            }
            Account::Contract(contract) => {
                s.begin_list(4);
                s.append(&CONTRACT_ACCOUNT_FLAG);
                s.append(&contract.nonce.to_be_bytes());
                s.append(&contract.storage_root.as_bytes().to_vec());

                let mut asset_list = Vec::with_capacity(contract.assets.len());

                for (id, balance) in contract.assets.iter() {
                    let asset = FixedContractAsset {
                        id:      id.clone(),
                        balance: balance.clone(),
                    };

                    asset_list.push(asset);
                }

                s.append_list(&asset_list);
            }
        }
    }
}

/// RLP decodable trait
impl rlp::Decodable for FixedAccount {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let flag: u8 = r.at(0)?.as_val()?;

        match flag {
            USER_ACCOUNT_FLAG => {
                let nonce = bytes_to_u64(r.at(1)?.data())?;
                let asset_list: Vec<FixedUserAssetInfo> = rlp::decode_list(r.at(2)?.as_raw());

                let mut assets = BTreeMap::new();

                for v in asset_list.into_iter() {
                    assets.insert(v.id, AssetInfo {
                        balance:  v.balance,
                        approved: v.approved,
                    });
                }

                Ok(FixedAccount {
                    inner: Account::User(UserAccount { nonce, assets }),
                })
            }
            CONTRACT_ACCOUNT_FLAG => {
                let nonce: u64 = r.at(1)?.as_val()?;
                let storage_root_bytes = r.at(2)?.data()?;
                let asset_list: Vec<FixedContractAsset> = rlp::decode_list(r.at(3)?.as_raw());

                let mut assets = BTreeMap::new();

                for v in asset_list {
                    assets.insert(v.id, v.balance);
                }

                let storage_root = MerkleRoot::from_bytes(Bytes::from(storage_root_bytes))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

                Ok(FixedAccount {
                    inner: Account::Contract(ContractAccount {
                        nonce,
                        assets,
                        storage_root,
                    }),
                })
            }
            _ => Err(rlp::DecoderError::RlpListLenWithZeroPrefix),
        }
    }
}

/// the `FixedUserAssetInfo` is a wrapper type of asset just to provide a
/// consistent serialization algorithm `rlp`.
#[derive(Clone, Debug)]
pub struct FixedUserAssetInfo {
    pub id:       AssetID,
    pub balance:  Balance,
    pub approved: BTreeMap<ContractAddress, ApprovedInfo>,
}

/// FixedAsset encodable to RLP
impl rlp::Encodable for FixedUserAssetInfo {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3);
        s.append(&self.id.as_bytes().to_vec());
        s.append(&self.balance.to_bytes_be());

        let mut info_list = Vec::with_capacity(self.approved.len());

        for (address, info) in self.approved.iter() {
            let fixed_info = FixedUserAssetApproved {
                contract_address: address.clone(),
                max:              info.max.clone(),
                used:             info.used.clone(),
            };
            info_list.push(fixed_info);
        }

        s.append_list(&info_list);
    }
}

/// RLP decodable trait
impl rlp::Decodable for FixedUserAssetInfo {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let id_bytes = r.at(0)?.data()?;
        let balance_bytes = r.at(1)?.data()?;
        let approved_list: Vec<FixedUserAssetApproved> = rlp::decode_list(r.at(2)?.as_raw());

        let mut approved_map = BTreeMap::new();
        for v in approved_list {
            approved_map.insert(v.contract_address, ApprovedInfo {
                max:  v.max,
                used: v.used,
            });
        }

        Ok(FixedUserAssetInfo {
            id:       AssetID::from_bytes(Bytes::from(id_bytes))
                .map_err(|_| rlp::DecoderError::RlpInvalidLength)?,
            balance:  Balance::from_bytes_be(balance_bytes),
            approved: approved_map,
        })
    }
}

/// the `FixedUserAssetApproved` is a wrapper type of asset just to provide a
/// consistent serialization algorithm `rlp`.
#[derive(Clone, Debug)]
pub struct FixedUserAssetApproved {
    pub contract_address: ContractAddress,
    pub max:              Balance,
    pub used:             Balance,
}

/// FixedAsset encodable to RLP
impl rlp::Encodable for FixedUserAssetApproved {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3);
        s.append(&self.contract_address.as_bytes().to_vec());
        s.append(&self.max.to_bytes_be());
        s.append(&self.used.to_bytes_be());
    }
}

/// RLP decodable trait
impl rlp::Decodable for FixedUserAssetApproved {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let address_bytes = r.at(0)?.data()?;
        let max_bytes = r.at(1)?.data()?;
        let used_bytes = r.at(2)?.data()?;

        Ok(FixedUserAssetApproved {
            contract_address: ContractAddress::from_bytes(Bytes::from(address_bytes))
                .map_err(|_| rlp::DecoderError::RlpInvalidLength)?,
            max:              Balance::from_bytes_be(max_bytes),
            used:             Balance::from_bytes_be(used_bytes),
        })
    }
}

/// the `FixedUserAssetApproved` is a wrapper type of asset just to provide a
/// consistent serialization algorithm `rlp`.
#[derive(Clone, Debug)]
pub struct FixedContractAsset {
    pub id:      AssetID,
    pub balance: Balance,
}

/// FixedAsset encodable to RLP
impl rlp::Encodable for FixedContractAsset {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2);
        s.append(&self.id.as_bytes().to_vec());
        s.append(&self.balance.to_bytes_be());
    }
}

/// RLP decodable trait
impl rlp::Decodable for FixedContractAsset {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let id_bytes = r.at(0)?.data()?;
        let balance_bytes = r.at(1)?.data()?;

        Ok(FixedContractAsset {
            id:      Hash::from_bytes(Bytes::from(id_bytes))
                .map_err(|_| rlp::DecoderError::RlpInvalidLength)?,
            balance: Balance::from_bytes_be(balance_bytes),
        })
    }
}

fn bytes_to_u64(bytes: &[u8]) -> u64 {
    let mut nonce_bytes = [0u8; 8];
    nonce_bytes.copy_from_slice(bytes);
    u64::from_be_bytes(nonce_bytes)
}

#[derive(Debug, Display, From)]
pub enum FixedTypesError {
    Decoder(rlp::DecoderError),
}

impl Error for FixedTypesError {}

impl From<FixedTypesError> for ProtocolError {
    fn from(err: FixedTypesError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
