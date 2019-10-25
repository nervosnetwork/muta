use bytes::Bytes;

use protocol::traits::executor::ContractSchema;
use protocol::types::{Account, Address, Asset, AssetID};

#[allow(dead_code)]
pub struct FixedBytesSchema;
impl ContractSchema for FixedBytesSchema {
    type Key = Bytes;
    type Value = Bytes;
}

pub struct FixedAssetSchema;
impl ContractSchema for FixedAssetSchema {
    type Key = AssetID;
    type Value = Asset;
}

pub struct FixedAccountSchema;
impl ContractSchema for FixedAccountSchema {
    type Key = Address;
    type Value = Account;
}
