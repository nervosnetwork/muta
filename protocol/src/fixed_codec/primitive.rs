use std::collections::BTreeMap;

use bytes::Bytes;

use crate::{
    ProtocolResult, impl_default_fixed_codec_for,
    fixed_codec::{FixedCodecError, ProtocolFixedCodec, bytes_to_u64},
    types::{Asset, AssetID, Fee, Balance, ContractAddress, Hash, Address, Account, AssetInfo, ApprovedInfo, UserAccount, MerkleRoot, ContractAccount},
};

// AssetID, MerkleRoot are alias of Hash type
impl ProtocolFixedCodec for Hash {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(self.as_bytes())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        AssetID::from_bytes(bytes)
    }
}

impl rlp::Encodable for Hash {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1)
            .append(&self.as_bytes().to_vec());
    }
}

impl rlp::Decodable for Hash {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let hash = Hash::from_bytes(Bytes::from(r.at(0)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        Ok(hash)
    }
}

impl ProtocolFixedCodec for Address {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        match self {
            Address::User(user) => Ok(user.as_bytes()),
            Address::Contract(contract) => Ok(contract.as_bytes()),
        }
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Address::from_bytes(bytes)
    }
}

impl_default_fixed_codec_for!(primitive, [Asset, Fee, Account]); 

impl rlp::Encodable for Asset {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(6)
            .append(&self.id.as_bytes().to_vec())
            .append(&self.manage_contract.as_bytes().to_vec())
            .append(&self.name.as_bytes())
            .append(&self.storage_root.as_bytes().to_vec())
            .append(&self.supply.to_bytes_be())
            .append(&self.symbol.as_bytes());
    }
}

impl rlp::Decodable for Asset {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 6 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
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
        s.begin_list(2)
            .append(&self.asset_id.as_bytes().to_vec())
            .append(&self.cycle.to_be_bytes().to_vec());
    }
}

impl rlp::Decodable for Fee {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let asset_id = Hash::from_bytes(Bytes::from(r.at(0)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let cycle = bytes_to_u64(r.at(1)?.data()?);

        Ok(Fee {
            asset_id,
            cycle
        })
    }
}

const USER_ACCOUNT_FLAG: u8 = 0;
const CONTRACT_ACCOUNT_FLAG: u8 = 1;

impl rlp::Encodable for Account {
    /// Append a value to the stream
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
                s.append(&user.nonce.to_be_bytes().to_vec());
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
                s.append(&contract.nonce.to_be_bytes().to_vec());
                s.append(&contract.storage_root.as_bytes().to_vec());
            }
        }
    }
}

/// RLP decodable trait
impl rlp::Decodable for Account {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let flag: u8 = r.at(0)?.as_val()?;

        match flag {
            USER_ACCOUNT_FLAG => {
                let asset_list: Vec<FixedUserAsset> = rlp::decode_list(r.at(1)?.as_raw());
                let nonce = bytes_to_u64(r.at(2)?.data()?);

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

                let nonce = bytes_to_u64(r.at(2)?.data()?);
                let storage_root_bytes = r.at(3)?.data()?;
                let storage_root = MerkleRoot::from_bytes(Bytes::from(storage_root_bytes))
                    .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

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
    /// Append a value to the stream
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
        s.append(&self.id.as_bytes().to_vec());
    }
}

/// RLP decodable trait
impl rlp::Decodable for FixedUserAsset {
    /// Decode a value from RLP bytes
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
        let id = AssetID::from_bytes(Bytes::from(r.at(2)?.data()?))
                .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

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

/// FixedAsset encodable to RLP
impl rlp::Encodable for FixedUserAssetApproved {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3)
            .append(&self.contract_address.as_bytes().to_vec())
            .append(&self.max.to_bytes_be())
            .append(&self.used.to_bytes_be());
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

#[derive(Clone, Debug)]
pub struct FixedContractAsset {
    pub id:      AssetID,
    pub balance: Balance,
}

impl rlp::Encodable for FixedContractAsset {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append(&self.balance.to_bytes_be())
            .append(&self.id.as_bytes().to_vec());
    }
}

/// RLP decodable trait
impl rlp::Decodable for FixedContractAsset {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let balance_bytes = r.at(0)?.data()?;
        let id_bytes = r.at(1)?.data()?;

        Ok(FixedContractAsset {
            id:      Hash::from_bytes(Bytes::from(id_bytes))
                .map_err(|_| rlp::DecoderError::RlpInvalidLength)?,
            balance: Balance::from_bytes_be(balance_bytes),
        })
    }
}
