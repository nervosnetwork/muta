use bytes::Bytes;

use crate::types::primitive::{
    AssetID, Balance, ContractAddress, ContractType, Fee, Hash, UserAddress,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawTransaction {
    pub chain_id: Hash,
    pub nonce:    Hash,
    pub timeout:  u64,
    pub fee:      Fee,
    pub action:   TransactionAction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransactionAction {
    Transfer {
        receiver:       UserAddress,
        carrying_asset: CarryingAsset,
    },
    Approve {
        spender:  ContractAddress,
        asset_id: AssetID,
        max:      Balance,
    },
    Deploy {
        code:          Bytes,
        contract_type: ContractType,
    },
    Call {
        contract:       ContractAddress,
        method:         String,
        args:           Vec<Bytes>,
        carrying_asset: Option<CarryingAsset>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarryingAsset {
    pub asset_id: AssetID,
    pub amount:   Balance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedTransaction {
    pub raw:       RawTransaction,
    pub tx_hash:   Hash,
    pub pubkey:    Bytes,
    pub signature: Bytes,
}
