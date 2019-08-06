use crate::types::primitive::{Address, Balance, Hash};

#[derive(Clone, Debug)]
pub struct RawTransaction {
    pub chain_id: Hash,
    pub nonce:    Hash,
    pub timeout:  u64,
    pub fee:      Fee,
    pub action:   TransactionAction,
}

#[derive(Clone, Debug)]
pub struct Fee {
    pub asset_id: Hash,
    pub cycle:    u64,
}

#[derive(Clone, Debug)]
pub enum TransactionAction {
    Transfer {
        receiver: Address,
        asset_id: Hash,
        amount:   Balance,
    },
    Approve {
        spender:  Address,
        asset_id: Hash,
        max:      Balance,
    },
    Deploy {
        code:          Vec<u8>,
        contract_type: ContractType,
    },
    Call {
        contract: Address,
        method:   String,
        args:     Vec<u8>,
        asset_id: Hash,
        amount:   Balance,
    },
}

#[derive(Clone, Debug)]
pub struct SignedTransaction {
    pub raw:       RawTransaction,
    pub tx_hash:   Hash,
    pub pubkey:    Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum ContractType {
    Asset,
    Library,
    App,
}
