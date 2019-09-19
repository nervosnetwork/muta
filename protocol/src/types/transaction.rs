use bytes::Bytes;

use crate::types::primitive::{
    AssetID, Balance, ContractAddress, ContractType, Fee, Hash, UserAddress,
};

#[derive(Clone, Debug)]
pub struct RawTransaction {
    pub chain_id: Hash,
    pub nonce:    Hash,
    pub timeout:  u64,
    pub fee:      Fee,
    pub action:   TransactionAction,
}

#[derive(Clone, Debug)]
pub enum TransactionAction {
    Transfer {
        receiver: UserAddress,
        asset_id: AssetID,
        amount:   Balance,
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
        contract: ContractAddress,
        method:   String,
        args:     Vec<Bytes>,
        asset_id: AssetID,
        amount:   Balance,
    },
}

#[derive(Clone, Debug)]
pub struct SignedTransaction {
    pub raw:       RawTransaction,
    pub tx_hash:   Hash,
    pub pubkey:    Bytes,
    pub signature: Bytes,
}
