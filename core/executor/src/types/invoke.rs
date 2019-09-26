use protocol::types::{AssetID, Balance, Address};

pub struct FixedTransferRequestSchema;

impl ContractSchema for FixedTransferRequestSchema {
    type Key = FixedTransferArgSchema;
    type Value = ();
}

#[derive(Clone, Debug)]
pub struct FixedTransferArgSchema {
    pub id: AssetID,
    pub to: Address,
    pub amount: Balance,
}
