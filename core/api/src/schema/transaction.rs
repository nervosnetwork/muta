use crate::schema::{Address, AssetID, Balance, Bytes, Hash, Uint64};

#[derive(GraphQLEnum, Clone)]
pub enum ContractType {
    // Asset contract
    Asset,
    // App contract, the code in the contract is allowed to change the state world.
    App,
    // Library contract, the code in the contract is not allowed to change the state world.
    Library,
}

// #####################
// GraphQLInputObject
// #####################

#[derive(GraphQLInputObject, Clone)]
#[graphql(description = "input raw transaction.")]
pub struct InputRawTransaction {
    pub chain_id:     Hash,
    pub fee_cycle:    Uint64,
    pub fee_asset_id: AssetID,
    pub nonce:        Hash,
    pub timeout:      Uint64,
}

#[derive(GraphQLInputObject, Clone)]
#[graphql(description = "input signature, hash, pubkey")]
pub struct InputTransactionEncryption {
    pub tx_hash:   Hash,
    pub pubkey:    Bytes,
    pub signature: Bytes,
}

#[derive(GraphQLInputObject, Clone)]
#[graphql(description = "input transfer action.")]
pub struct InputTransferAction {
    pub carrying_amount:   Balance,
    pub carrying_asset_id: AssetID,
    pub receiver:          Address,
}

#[derive(GraphQLInputObject, Clone)]
#[graphql(description = "input deploy action.")]
pub struct InputDeployAction {
    pub code:          Bytes,
    pub contract_type: ContractType,
}
