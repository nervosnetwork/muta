use bytes::Bytes;

use crate::types::{
    AssetID, Balance, Bloom, ContractAddress, ContractType, Fee, Hash, MerkleRoot, UserAddress,
};

#[derive(Clone, Debug)]
pub struct Receipt {
    pub state_root:  MerkleRoot,
    pub epoch_id:    u64,
    pub tx_hash:     Hash,
    pub cycles_used: Fee,
    pub result:      ReceiptResult,
}

#[derive(Clone, Debug)]
pub enum ReceiptResult {
    Transfer {
        receiver:      UserAddress,
        asset_id:      AssetID,
        before_amount: Balance,
        after_amount:  Balance,
    },
    Approve {
        spender:  ContractAddress,
        asset_id: AssetID,
        max:      Balance,
    },
    Deploy {
        contract:      ContractAddress,
        contract_type: ContractType,
    },
    Call {
        contract:     ContractAddress,
        return_value: Bytes,
        logs_bloom:   Box<Bloom>,
    },
    Fail {
        system: String,
        user:   String,
    },
}
