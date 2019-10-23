use std::collections::BTreeMap;

use bytes::Bytes;

use crate::fixed_codec::{FixedCodecError, ProtocolFixedCodec};
use crate::types::{
    Account, Address, ApprovedInfo, Asset, AssetID, AssetInfo, Balance, ContractAccount,
    ContractAddress, Fee, Hash, UserAccount, UserAddress,
};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl ProtocolFixedCodec trait for types
impl_default_fixed_codec_for!(primitive, [
    Hash,
    Asset,
    Fee,
    Address,
    UserAddress,
    ContractAddress,
    Account
]);

// AssetID, MerkleRoot are alias of Hash type
impl rlp::Encodable for Hash {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1).append(&self.as_bytes().to_vec());
    }
}

impl rlp::Decodable for Hash {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let hash = Hash::from_bytes(Bytes::from(r.at(0)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        Ok(hash)
    }
}

impl rlp::Encodable for Asset {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(6)
            .append(&self.id)
            .append(&self.manage_contract)
            .append(&self.name.as_bytes())
            .append(&self.storage_root)
            .append(&self.supply.to_bytes_be())
            .append(&self.symbol.as_bytes());
    }
}

impl rlp::Decodable for Asset {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 6 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let id: Hash = rlp::decode(r.at(0)?.as_raw())?;
        let manage_contract = rlp::decode(r.at(1)?.as_raw())?;
        let name = String::from_utf8(r.at(2)?.data()?.to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let storage_root: Hash = rlp::decode(r.at(3)?.as_raw())?;
        let supply = Balance::from_bytes_be(r.at(4)?.data()?);
        let symbol = String::from_utf8(r.at(5)?.data()?.to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(Asset {
            id,
            manage_contract,
            name,
            storage_root,
            supply,
            symbol,
        })
    }
}

impl rlp::Encodable for Fee {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2).append(&self.asset_id).append(&self.cycle);
    }
}

impl rlp::Decodable for Fee {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let asset_id: Hash = rlp::decode(r.at(0)?.as_raw())?;
        let cycle = r.at(1)?.as_val()?;

        Ok(Fee { asset_id, cycle })
    }
}

impl rlp::Encodable for Address {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        match self {
            Address::User(user) => {
                s.begin_list(1).append(&user.as_bytes().to_vec());
            }
            Address::Contract(contract) => {
                s.begin_list(1).append(&contract.as_bytes().to_vec());
            }
        }
    }
}

impl rlp::Decodable for Address {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let address = Address::from_bytes(Bytes::from(r.at(0)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(address)
    }
}

impl rlp::Encodable for UserAddress {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1).append(&self.as_bytes().to_vec());
    }
}

impl rlp::Decodable for UserAddress {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let address = UserAddress::from_bytes(Bytes::from(r.at(0)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(address)
    }
}

impl rlp::Encodable for ContractAddress {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1).append(&self.as_bytes().to_vec());
    }
}

impl rlp::Decodable for ContractAddress {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let address = ContractAddress::from_bytes(Bytes::from(r.at(0)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(address)
    }
}

const USER_ACCOUNT_FLAG: u8 = 0;
const CONTRACT_ACCOUNT_FLAG: u8 = 1;

impl rlp::Encodable for Account {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        match self {
            Account::User(user) => {
                s.begin_list(3);
                s.append(&USER_ACCOUNT_FLAG);

                let mut asset_list = Vec::with_capacity(user.assets.len());

                for (id, asset_info) in user.assets.iter() {
                    let asset_info = FixedUserAsset {
                        id:       id.clone(),
                        balance:  asset_info.balance.clone(),
                        approved: asset_info.approved.clone(),
                    };

                    asset_list.push(asset_info);
                }

                s.append_list(&asset_list);
                s.append(&user.nonce);
            }
            Account::Contract(contract) => {
                s.begin_list(4);
                s.append(&CONTRACT_ACCOUNT_FLAG);

                let mut asset_list = Vec::with_capacity(contract.assets.len());

                for (id, balance) in contract.assets.iter() {
                    let asset = FixedContractAsset {
                        id:      id.clone(),
                        balance: balance.clone(),
                    };

                    asset_list.push(asset);
                }

                s.append_list(&asset_list);
                s.append(&contract.nonce);
                s.append(&contract.storage_root);
            }
        }
    }
}

impl rlp::Decodable for Account {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let flag: u8 = r.at(0)?.as_val()?;

        match flag {
            USER_ACCOUNT_FLAG => {
                let asset_list: Vec<FixedUserAsset> = rlp::decode_list(r.at(1)?.as_raw());
                let nonce = r.at(2)?.as_val()?;

                let mut assets = BTreeMap::new();

                for v in asset_list.into_iter() {
                    assets.insert(v.id, AssetInfo {
                        balance:  v.balance,
                        approved: v.approved,
                    });
                }

                Ok(Account::User(UserAccount { nonce, assets }))
            }
            CONTRACT_ACCOUNT_FLAG => {
                let asset_list: Vec<FixedContractAsset> = rlp::decode_list(r.at(1)?.as_raw());

                let mut assets = BTreeMap::new();

                for v in asset_list {
                    assets.insert(v.id, v.balance);
                }

                let nonce = r.at(2)?.as_val()?;
                let storage_root: Hash = rlp::decode(r.at(3)?.as_raw())?;

                Ok(Account::Contract(ContractAccount {
                    nonce,
                    assets,
                    storage_root,
                }))
            }
            _ => Err(rlp::DecoderError::RlpListLenWithZeroPrefix),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FixedUserAsset {
    pub id:       AssetID,
    pub balance:  Balance,
    pub approved: BTreeMap<ContractAddress, ApprovedInfo>,
}

impl rlp::Encodable for FixedUserAsset {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3);

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
        s.append(&self.balance.to_bytes_be());
        s.append(&self.id);
    }
}

impl rlp::Decodable for FixedUserAsset {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let approved_list: Vec<FixedUserAssetApproved> = rlp::decode_list(r.at(0)?.as_raw());

        let mut approved = BTreeMap::new();
        for v in approved_list {
            approved.insert(v.contract_address, ApprovedInfo {
                max:  v.max,
                used: v.used,
            });
        }

        let balance = Balance::from_bytes_be(r.at(1)?.data()?);
        let id = rlp::decode(r.at(2)?.as_raw())?;
        Ok(FixedUserAsset {
            id,
            balance,
            approved,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FixedUserAssetApproved {
    pub contract_address: ContractAddress,
    pub max:              Balance,
    pub used:             Balance,
}

impl rlp::Encodable for FixedUserAssetApproved {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3)
            .append(&self.contract_address)
            .append(&self.max.to_bytes_be())
            .append(&self.used.to_bytes_be());
    }
}

impl rlp::Decodable for FixedUserAssetApproved {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let contract_address = rlp::decode(r.at(0)?.as_raw())?;
        let max = Balance::from_bytes_be(r.at(1)?.data()?);
        let used = Balance::from_bytes_be(r.at(2)?.data()?);

        Ok(FixedUserAssetApproved {
            contract_address,
            max,
            used,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FixedContractAsset {
    pub id:      AssetID,
    pub balance: Balance,
}

impl rlp::Encodable for FixedContractAsset {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append(&self.balance.to_bytes_be())
            .append(&self.id);
    }
}

impl rlp::Decodable for FixedContractAsset {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let balance = Balance::from_bytes_be(r.at(0)?.data()?);
        let id = rlp::decode(r.at(1)?.as_raw())?;

        Ok(FixedContractAsset { id, balance })
    }
}
