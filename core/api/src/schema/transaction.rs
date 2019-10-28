use crate::schema::{Address, AssetID, Balance, Bytes, Hash, Uint64};

#[derive(GraphQLEnum, Clone)]
#[graphql(description = "According to different purposes, Muta has many contract type")]
pub enum ContractType {
    // Asset contract
    #[graphql(
        description = "Asset contract often use for creating User Define Asset(also known as UDT(User Define Token))"
    )]
    Asset,
    // App contract, the code in the contract is allowed to change the state world.
    #[graphql(
        description = "App contract often use for creating DAPP(Decentralized APPlication) "
    )]
    App,
    // Library contract, the code in the contract is not allowed to change the state world.
    #[graphql(description = "Library contract often providing reusable and immutable function")]
    Library,
}

// #####################
// GraphQLInputObject
// #####################

#[derive(GraphQLInputObject, Clone)]
#[graphql(description = "There was many types of transaction in Muta, \
                         A transaction often require computing resources or write data to chain,\
                         these resources are valuable so we need to pay some token for them.\
                         InputRawTransaction describes information above")]
pub struct InputRawTransaction {
    #[graphql(description = "Identifier of the chain.")]
    pub chain_id: Hash,
    #[graphql(
        description = "Mostly like the gas limit in Ethereum, describes the fee that \
                       you are willing to pay the highest price for the transaction"
    )]
    pub fee_cycle: Uint64,
    #[graphql(description = "asset type")]
    pub fee_asset_id: AssetID,
    #[graphql(
        description = "Every transaction has its own id, unlike Ethereum's nonce,\
                       the nonce in Muta is an hash"
    )]
    pub nonce: Hash,
    #[graphql(description = "For security and performance reasons, \
    Muta will only deal with trade request over a period of time,\
    the `timeout` should be `timeout > current_epoch_height` and `timeout < current_epoch_height + timeout_gap`,\
    the `timeout_gap` generally equal to 20.")]
    pub timeout: Uint64,
}

#[derive(GraphQLInputObject, Clone)]
#[graphql(description = "Signature of the transaction")]
pub struct InputTransactionEncryption {
    #[graphql(description = "The digest of the transaction")]
    pub tx_hash: Hash,
    #[graphql(description = "The public key of transfer")]
    pub pubkey: Bytes,
    #[graphql(description = "The signature of the transaction")]
    pub signature: Bytes,
}

#[derive(GraphQLInputObject, Clone)]
#[graphql(description = "The action of transfer transaction")]
pub struct InputTransferAction {
    #[graphql(description = "The amount of the transfer")]
    pub carrying_amount: Balance,
    #[graphql(description = "The asset of of the transfer")]
    pub carrying_asset_id: AssetID,
    #[graphql(description = "The receiver of the transfer")]
    pub receiver: Address,
}

#[derive(GraphQLInputObject, Clone)]
#[graphql(description = "The deploy transfer transaction")]
pub struct InputDeployAction {
    #[graphql(description = "Encoded contract code")]
    pub code: Bytes,
    #[graphql(description = "The type of contract")]
    pub contract_type: ContractType,
}
