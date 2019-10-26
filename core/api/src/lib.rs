#[macro_use]
extern crate juniper_codegen;

pub mod adapter;
pub mod config;
mod schema;

use std::convert::TryFrom;
use std::sync::Arc;
use std::u64;

use futures::executor::block_on;
use juniper::graphiql::graphiql_source;
use juniper::{FieldError, FieldResult};
use tide::{error::ResultExt, response, App, EndpointResult};

use common_crypto::{HashValue, PrivateKey, PublicKey, Secp256k1PrivateKey, Signature};
use protocol::fixed_codec::ProtocolFixedCodec;
use protocol::traits::{APIAdapter, Context};

use crate::config::GraphQLConfig;
use crate::schema::{
    Address, AssetID, Balance, Bytes, ContractType, Epoch, Hash, InputDeployAction,
    InputRawTransaction, InputTransactionEncryption, InputTransferAction, Uint64,
};

pub async fn start_graphql<Adapter: APIAdapter + 'static>(cfg: GraphQLConfig, adapter: Adapter) {
    let state = State {
        adapter: Arc::new(Box::new(adapter)),
    };

    let mut app = App::with_state(Arc::new(state));
    app.at(&cfg.graphql_uri).post(handle_graphql);
    app.at(&cfg.graphiql_uri).get(handle_graphiql);
    app.serve(cfg.listening_address).await.unwrap();
}

// This is accessible as state in Tide, and as executor context in Juniper.
#[derive(Clone)]
struct State {
    adapter: Arc<Box<dyn APIAdapter>>,
}

impl juniper::Context for State {}

// We define `Query` unit struct here. GraphQL queries will refer to this
// struct. The struct itself doesn't have any associated state (and there's no
// need to do so), but instead it exposes the accumulator state from the
// context.
struct Query;
// Switch to async/await fn https://github.com/graphql-rust/juniper/issues/2
#[juniper::object(Context = State)]
impl Query {
    #[graphql(name = "getLatestEpoch", description = "Get the latest epoch")]
    fn get_latest_epoch(state_ctx: &State, epoch_id: Option<Uint64>) -> FieldResult<Epoch> {
        let epoch_id = opt_hex_to_u64(epoch_id.map(|id| id.as_hex()))?;

        let epoch = block_on(state_ctx.adapter.get_epoch_by_id(Context::new(), epoch_id))
            .map_err(FieldError::from)?;
        Ok(Epoch::from(epoch))
    }

    #[graphql(
        name = "getBalance",
        description = "Get the asset balance of an account",
        arguments(id(description = "The asset id. Asset is the first-class in Muta, \
            this means that your assets can be more than one in Muta, \
            and the UDT(User Defined Token) will be supported in the future"))
    )]
    fn get_balance(
        state_ctx: &State,
        address: Address,
        id: AssetID,
        epoch_id: Option<Uint64>,
    ) -> FieldResult<Balance> {
        let epoch_id = opt_hex_to_u64(epoch_id.map(|id| id.as_hex()))?;
        let address = protocol::types::Address::from_hex(&address.as_hex())?;
        let id = protocol::types::AssetID::from_hex(&id.as_hex())?;

        let balance = block_on(state_ctx.adapter.get_balance(
            Context::new(),
            &address,
            &id,
            epoch_id,
        ))
        .map_err(FieldError::from)?;
        Ok(Balance::from(balance))
    }
}

struct Mutation;
// Switch to async/await fn https://github.com/graphql-rust/juniper/issues/2
#[juniper::object(Context = State)]
impl Mutation {
    #[graphql(
        name = "sendTransferTransaction",
        description = "Send a transfer transaction to the blockchain."
    )]
    fn send_transfer_transaction(
        state_ctx: &State,
        input_raw: InputRawTransaction,
        input_action: InputTransferAction,
        input_encryption: InputTransactionEncryption,
    ) -> FieldResult<Hash> {
        let action = cover_transfer_action(&input_action)?;
        let signed_tx = cover_to_signed_tx(&action, &input_raw, &input_encryption)?;
        block_on(
            state_ctx
                .adapter
                .insert_signed_txs(Context::new(), signed_tx),
        )
        .map_err(FieldError::from)?;

        Ok(input_encryption.tx_hash)
    }

    #[graphql(
        name = "sendDeployTransaction",
        description = "Send deployment contract transaction to the blockchain."
    )]
    fn send_deploy_transaction(
        state_ctx: &State,
        input_raw: InputRawTransaction,
        input_action: InputDeployAction,
        input_encryption: InputTransactionEncryption,
    ) -> FieldResult<Hash> {
        let action = cover_deploy_action(&input_action)?;
        let signed_tx = cover_to_signed_tx(&action, &input_raw, &input_encryption)?;
        block_on(
            state_ctx
                .adapter
                .insert_signed_txs(Context::new(), signed_tx),
        )
        .map_err(FieldError::from)?;

        Ok(input_encryption.tx_hash)
    }

    #[graphql(
        name = "sendUnsafeTransferTransaction",
        deprecated = "DON'T use it in production! This is just for development."
    )]
    fn send_unsafe_transfer_transaction(
        state_ctx: &State,
        input_raw: InputRawTransaction,
        input_action: InputTransferAction,
        input_privkey: Bytes,
    ) -> FieldResult<Hash> {
        let action = cover_transfer_action(&input_action)?;
        let raw_tx = cover_to_raw_tx(&action, &input_raw)?;
        let tx_hash = protocol::types::Hash::digest(raw_tx.encode_fixed()?);
        let tx_hash = Hash::from(tx_hash);

        let input_encryption = gen_input_tx_encryption(input_privkey, tx_hash.clone())?;
        let signed_tx = cover_to_signed_tx(&action, &input_raw, &input_encryption)?;
        block_on(
            state_ctx
                .adapter
                .insert_signed_txs(Context::new(), signed_tx),
        )
        .map_err(FieldError::from)?;

        Ok(tx_hash)
    }

    #[graphql(
        name = "sendUnsafeDeployTransaction",
        deprecated = "DON'T use it in production! This is just for development."
    )]
    fn send_unsafe_deploy_transaction(
        state_ctx: &State,
        input_raw: InputRawTransaction,
        input_action: InputDeployAction,
        input_privkey: Bytes,
    ) -> FieldResult<Hash> {
        let action = cover_deploy_action(&input_action)?;
        let raw_tx = cover_to_raw_tx(&action, &input_raw)?;
        let tx_hash = protocol::types::Hash::digest(raw_tx.encode_fixed()?);
        let tx_hash = Hash::from(tx_hash);

        let input_encryption = gen_input_tx_encryption(input_privkey, tx_hash.clone())?;
        let signed_tx = cover_to_signed_tx(&action, &input_raw, &input_encryption)?;
        block_on(
            state_ctx
                .adapter
                .insert_signed_txs(Context::new(), signed_tx),
        )
        .map_err(FieldError::from)?;

        Ok(tx_hash)
    }
}

// Adding `Query` and `Mutation` together we get `Schema`, which describes,
// well, the whole GraphQL schema.
type Schema = juniper::RootNode<'static, Query, Mutation>;

// Finally, we'll bridge between Tide and Juniper. `GraphQLRequest` from Juniper
// implements `Deserialize`, so we use `Json` extractor to deserialize the
// request body.
async fn handle_graphql(mut ctx: tide::Context<Arc<State>>) -> EndpointResult {
    let query: juniper::http::GraphQLRequest = ctx.body_json().await.client_err()?;
    let schema = Schema::new(Query, Mutation);
    let response = query.execute(&schema, ctx.state());
    let status = if response.is_ok() {
        http::status::StatusCode::OK
    } else {
        http::status::StatusCode::BAD_REQUEST
    };
    let mut resp = response::json(response);
    *resp.status_mut() = status;
    Ok(resp)
}

async fn handle_graphiql(
    _ctx: tide::Context<Arc<State>>,
) -> EndpointResult<tide::http::Response<String>> {
    let html = graphiql_source("/graphql");

    Ok(tide::http::Response::builder()
        .status(http::status::StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(html)
        .unwrap())
}

fn hex_to_vec_u8(s: &str) -> FieldResult<Vec<u8>> {
    hex::decode(s).map_err(FieldError::from)
}

fn hex_to_u64(s: &str) -> FieldResult<u64> {
    let n = u64::from_str_radix(s, 16).map_err(FieldError::from)?;
    Ok(n)
}

fn opt_hex_to_u64(s: Option<String>) -> FieldResult<Option<u64>> {
    match s {
        Some(s) => match hex_to_u64(&s) {
            Ok(num) => Ok(Some(num)),
            Err(e) => Err(e),
        },
        None => Ok(None),
    }
}

fn gen_input_tx_encryption(
    input_privkey: Bytes,
    tx_hash: Hash,
) -> FieldResult<InputTransactionEncryption> {
    let privkey = Secp256k1PrivateKey::try_from(hex_to_vec_u8(&input_privkey.as_hex())?.as_ref())
        .map_err(FieldError::from)?;
    let pubkey = privkey.pub_key();
    let hash_value = HashValue::try_from(hex_to_vec_u8(&tx_hash.as_hex())?.as_ref())
        .map_err(FieldError::from)?;
    let signature = privkey.sign_message(&hash_value);

    let input_encryption = InputTransactionEncryption {
        tx_hash:   tx_hash.clone(),
        pubkey:    Bytes::from(pubkey.to_bytes()),
        signature: Bytes::from(signature.to_bytes()),
    };
    Ok(input_encryption)
}

// #####################
// Convert from graphql type to protocol type
// #####################

fn cover_to_raw_tx(
    action: &protocol::types::TransactionAction,
    input_raw: &InputRawTransaction,
) -> FieldResult<protocol::types::RawTransaction> {
    let raw = protocol::types::RawTransaction {
        chain_id: protocol::types::Hash::from_hex(&input_raw.chain_id.as_hex())
            .map_err(FieldError::from)?,
        nonce:    protocol::types::Hash::from_hex(&input_raw.nonce.as_hex())
            .map_err(FieldError::from)?,
        timeout:  hex_to_u64(&input_raw.timeout.as_hex())?,
        fee:      protocol::types::Fee {
            asset_id: protocol::types::AssetID::from_hex(&input_raw.fee_asset_id.as_hex())
                .map_err(FieldError::from)?,
            cycle:    hex_to_u64(&input_raw.fee_cycle.as_hex())?,
        },
        action:   action.clone(),
    };

    Ok(raw)
}

fn cover_to_signed_tx(
    action: &protocol::types::TransactionAction,
    input_raw: &InputRawTransaction,
    input_encryption: &InputTransactionEncryption,
) -> FieldResult<protocol::types::SignedTransaction> {
    let raw = cover_to_raw_tx(action, input_raw)?;

    let signed_tx = protocol::types::SignedTransaction {
        raw,
        tx_hash: protocol::types::Hash::from_hex(&input_encryption.tx_hash.as_hex())
            .map_err(FieldError::from)?,
        pubkey: bytes::Bytes::from(hex_to_vec_u8(&input_encryption.pubkey.as_hex())?),
        signature: bytes::Bytes::from(hex_to_vec_u8(&input_encryption.signature.as_hex())?),
    };

    Ok(signed_tx)
}

fn cover_transfer_action(
    input_action: &InputTransferAction,
) -> FieldResult<protocol::types::TransactionAction> {
    let action = protocol::types::TransactionAction::Transfer {
        receiver:       protocol::types::UserAddress::from_hex(&input_action.receiver.as_hex())
            .map_err(FieldError::from)?,
        carrying_asset: protocol::types::CarryingAsset {
            asset_id: protocol::types::AssetID::from_hex(&input_action.carrying_asset_id.as_hex())
                .map_err(FieldError::from)?,
            amount:   protocol::types::Balance::from_bytes_be(
                hex_to_vec_u8(&input_action.carrying_amount.as_hex())?.as_ref(),
            ),
        },
    };

    Ok(action)
}

fn cover_deploy_action(
    input_action: &InputDeployAction,
) -> FieldResult<protocol::types::TransactionAction> {
    let contract_type = match input_action.contract_type {
        ContractType::Asset => protocol::types::ContractType::Asset,
        ContractType::App => protocol::types::ContractType::App,
        ContractType::Library => protocol::types::ContractType::Library,
    };

    let action = protocol::types::TransactionAction::Deploy {
        code: bytes::Bytes::from(hex_to_vec_u8(&input_action.code.as_hex())?),
        contract_type,
    };

    Ok(action)
}
