use protocol::traits::executor::ContractSchema;
use protocol::types::{Account, Address, Asset, AssetID};

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
