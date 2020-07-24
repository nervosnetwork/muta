use protocol::ProtocolResult;

use crate::schema::{Address, Bytes, Hash, SchemaError, Uint64};

#[derive(juniper::GraphQLObject, Clone)]
pub struct SignedTransaction {
    pub chain_id:     Hash,
    pub cycles_limit: Uint64,
    pub cycles_price: Uint64,
    pub nonce:        Hash,
    pub timeout:      Uint64,
    pub sender:       Address,
    pub service_name: String,
    pub method:       String,
    pub payload:      String,
    pub tx_hash:      Hash,
    pub pubkey:       Bytes,
    pub signature:    Bytes,
}

impl From<protocol::types::SignedTransaction> for SignedTransaction {
    fn from(stx: protocol::types::SignedTransaction) -> Self {
        Self {
            chain_id:     Hash::from(stx.raw.chain_id),
            cycles_limit: Uint64::from(stx.raw.cycles_limit),
            cycles_price: Uint64::from(stx.raw.cycles_price),
            nonce:        Hash::from(stx.raw.nonce),
            timeout:      Uint64::from(stx.raw.timeout),
            sender:       Address::from(stx.raw.sender),
            service_name: stx.raw.request.service_name,
            method:       stx.raw.request.method,
            payload:      stx.raw.request.payload,
            tx_hash:      Hash::from(stx.tx_hash),
            pubkey:       Bytes::from(stx.pubkey),
            signature:    Bytes::from(stx.signature),
        }
    }
}

// #####################
// GraphQLInputObject
// #####################

#[derive(juniper::GraphQLInputObject, Clone)]
#[graphql(description = "There was many types of transaction in Muta, \
                         A transaction often require computing resources or write data to chain,\
                         these resources are valuable so we need to pay some token for them.\
                         InputRawTransaction describes information above")]
pub struct InputRawTransaction {
    #[graphql(description = "Identifier of the chain.")]
    pub chain_id:     Hash,
    #[graphql(
        description = "Mostly like the gas limit in Ethereum, describes the fee that \
                       you are willing to pay the highest price for the transaction"
    )]
    pub cycles_limit: Uint64,
    pub cycles_price: Uint64,
    #[graphql(
        description = "Every transaction has its own id, unlike Ethereum's nonce,\
                       the nonce in Muta is an hash"
    )]
    pub nonce:        Hash,
    #[graphql(description = "For security and performance reasons, \
    Muta will only deal with trade request over a period of time,\
    the `timeout` should be `timeout > current_block_height` and `timeout < current_block_height + timeout_gap`,\
    the `timeout_gap` generally equal to 20.")]
    pub timeout:      Uint64,
    pub service_name: String,
    pub method:       String,
    pub payload:      String,
    pub sender:       Address,
}

#[derive(juniper::GraphQLInputObject, Clone)]
#[graphql(description = "Signature of the transaction")]
pub struct InputTransactionEncryption {
    #[graphql(description = "The digest of the transaction")]
    pub tx_hash:   Hash,
    #[graphql(description = "The public key of transfer")]
    pub pubkey:    Bytes,
    #[graphql(description = "The signature of the transaction")]
    pub signature: Bytes,
}

pub fn to_signed_transaction(
    raw: InputRawTransaction,
    encryption: InputTransactionEncryption,
) -> ProtocolResult<protocol::types::SignedTransaction> {
    let pubkey: &[u8] = &hex::decode(encryption.pubkey.as_hex()?).map_err(SchemaError::from)?;
    let signature: &[u8] =
        &hex::decode(encryption.signature.as_hex()?).map_err(SchemaError::from)?;

    Ok(protocol::types::SignedTransaction {
        raw:       to_transaction(raw)?,
        tx_hash:   protocol::types::Hash::from_hex(&encryption.tx_hash.as_hex())?,
        pubkey:    bytes::BytesMut::from(pubkey).freeze(),
        signature: bytes::BytesMut::from(signature).freeze(),
    })
}

pub fn to_transaction(raw: InputRawTransaction) -> ProtocolResult<protocol::types::RawTransaction> {
    Ok(protocol::types::RawTransaction {
        chain_id:     protocol::types::Hash::from_hex(&raw.chain_id.as_hex())?,
        nonce:        protocol::types::Hash::from_hex(&raw.nonce.as_hex())?,
        timeout:      raw.timeout.try_into_u64()?,
        cycles_price: raw.cycles_price.try_into_u64()?,
        cycles_limit: raw.cycles_limit.try_into_u64()?,
        request:      protocol::types::TransactionRequest {
            service_name: raw.service_name.to_owned(),
            method:       raw.method.to_owned(),
            payload:      raw.payload.to_owned(),
        },
        sender:       raw.sender.to_str().parse()?,
    })
}
