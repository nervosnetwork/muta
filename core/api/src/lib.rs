#[macro_use]
extern crate juniper_codegen;

pub mod adapter;
pub mod config;
mod schema;

use std::convert::TryFrom;
use std::sync::Arc;

use futures::executor::block_on;
use http::status::StatusCode;
use juniper::graphiql::graphiql_source;
use juniper::{FieldError, FieldResult};
use tide::{Request, Response, ResultExt, Server};

use common_crypto::{
    HashValue, PrivateKey, PublicKey, Secp256k1PrivateKey, Signature, ToPublicKey,
};
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{APIAdapter, Context};

use crate::config::GraphQLConfig;
use crate::schema::{
    to_signed_transaction, to_transaction, Address, Bytes, Epoch, ExecResp, Hash,
    InputRawTransaction, InputTransactionEncryption, Receipt, SignedTransaction, Uint64,
};

// This is accessible as state in Tide, and as executor context in Juniper.
#[derive(Clone)]
struct State {
    adapter: Arc<Box<dyn APIAdapter>>,
}

// We define `Query` unit struct here. GraphQL queries will refer to this
// struct. The struct itself doesn't have any associated state (and there's no
// need to do so), but instead it exposes the accumulator state from the
// context.
struct Query;
// Switch to async/await fn https://github.com/graphql-rust/juniper/issues/2
#[juniper::object(Context = State)]
impl Query {
    #[graphql(name = "getEpoch", description = "Get the epoch")]
    fn get_latest_epoch(state_ctx: &State, epoch_id: Option<Uint64>) -> FieldResult<Epoch> {
        let epoch_id = match epoch_id {
            Some(id) => Some(id.try_into_u64()?),
            None => None,
        };

        let epoch = block_on(state_ctx.adapter.get_epoch_by_id(Context::new(), epoch_id))?;
        Ok(Epoch::from(epoch))
    }

    #[graphql(name = "getTransaction", description = "Get the transaction by hash")]
    fn get_transaction(state_ctx: &State, tx_hash: Hash) -> FieldResult<SignedTransaction> {
        let hash = protocol::types::Hash::from_hex(&tx_hash.as_hex())?;
        let stx = block_on(
            state_ctx
                .adapter
                .get_transaction_by_hash(Context::new(), hash),
        )?;
        Ok(SignedTransaction::from(stx))
    }

    #[graphql(
        name = "getReceipt",
        description = "Get the receipt by transaction hash"
    )]
    fn get_receipt(state_ctx: &State, tx_hash: Hash) -> FieldResult<Receipt> {
        let hash = protocol::types::Hash::from_hex(&tx_hash.as_hex())?;
        let receipt = block_on(
            state_ctx
                .adapter
                .get_receipt_by_tx_hash(Context::new(), hash),
        )?;

        Ok(Receipt::from(receipt))
    }

    #[graphql(name = "queryService", description = "query service")]
    fn query_service(
        state_ctx: &State,
        epoch_id: Option<Uint64>,
        cycels_limit: Option<Uint64>,
        cycles_price: Option<Uint64>,
        caller: Address,
        service_name: String,
        method: String,
        payload: String,
    ) -> FieldResult<ExecResp> {
        let epoch_id = match epoch_id {
            Some(id) => id.try_into_u64()?,
            None => {
                block_on(state_ctx.adapter.get_epoch_by_id(Context::new(), None))?
                    .header
                    .epoch_id
            }
        };
        let cycels_limit = match cycels_limit {
            Some(cycels_limit) => cycels_limit.try_into_u64()?,
            None => std::u64::MAX,
        };

        let cycles_price = match cycles_price {
            Some(cycles_price) => cycles_price.try_into_u64()?,
            None => 1,
        };

        let address = protocol::types::Address::from_hex(&caller.as_hex())?;

        let exec_resp = block_on(state_ctx.adapter.query_service(
            Context::new(),
            epoch_id,
            cycels_limit,
            cycles_price,
            address,
            service_name,
            method,
            payload,
        ))?;
        Ok(ExecResp::from(exec_resp))
    }
}

struct Mutation;
// Switch to async/await fn https://github.com/graphql-rust/juniper/issues/2
#[juniper::object(Context = State)]
impl Mutation {
    #[graphql(name = "sendTransaction", description = "send transaction")]
    fn send_transaction(
        state_ctx: &State,
        input_raw: InputRawTransaction,
        input_encryption: InputTransactionEncryption,
    ) -> FieldResult<Hash> {
        let stx = to_signed_transaction(input_raw, input_encryption)?;
        let tx_hash = stx.tx_hash.clone();

        block_on(state_ctx.adapter.insert_signed_txs(Context::new(), stx))
            .map_err(FieldError::from)?;

        Ok(Hash::from(tx_hash))
    }

    #[graphql(
        name = "unsafeSendTransaction",
        deprecated = "DON'T use it in production! This is just for development."
    )]
    fn unsafe_send_transaction(
        state_ctx: &State,
        input_raw: InputRawTransaction,
        input_privkey: Bytes,
    ) -> FieldResult<Hash> {
        let raw_tx = to_transaction(input_raw)?;
        let tx_hash = protocol::types::Hash::digest(raw_tx.encode_fixed()?);

        let privkey = Secp256k1PrivateKey::try_from(input_privkey.to_vec()?.as_ref())?;
        let pubkey = privkey.pub_key();
        let hash_value = HashValue::try_from(tx_hash.as_bytes().as_ref())?;
        let signature = privkey.sign_message(&hash_value);

        let stx = protocol::types::SignedTransaction {
            raw:       raw_tx,
            tx_hash:   tx_hash.clone(),
            signature: signature.to_bytes(),
            pubkey:    pubkey.to_bytes(),
        };
        block_on(state_ctx.adapter.insert_signed_txs(Context::new(), stx))
            .map_err(FieldError::from)?;

        Ok(Hash::from(tx_hash))
    }
}

// Adding `Query` and `Mutation` together we get `Schema`, which describes,
// well, the whole GraphQL schema.
type Schema = juniper::RootNode<'static, Query, Mutation>;

// Finally, we'll bridge between Tide and Juniper. `GraphQLRequest` from Juniper
// implements `Deserialize`, so we use `Json` extractor to deserialize the
// request body.
async fn handle_graphql(mut req: Request<State>) -> Response {
    let query: juniper::http::GraphQLRequest = match req.body_json().await.client_err() {
        Ok(query) => query,
        Err(e) => {
            return Response::new(StatusCode::BAD_REQUEST.into()).body_string(format!("{:?}", e))
        }
    };

    let schema = Schema::new(Query, Mutation);
    let response = query.execute(&schema, req.state());
    let status = if response.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };

    Response::new(status.into())
        .body_json(&response)
        .expect("Json parsing errors on return should never occur.")
}

async fn handle_graphiql(_: Request<State>) -> Response {
    let html = graphiql_source("/graphql");

    Response::new(StatusCode::OK.into())
        .body_string(html)
        .set_header("Content-Type", "text/html")
}

pub async fn start_graphql<Adapter: APIAdapter + 'static>(cfg: GraphQLConfig, adapter: Adapter) {
    let state = State {
        adapter: Arc::new(Box::new(adapter)),
    };

    let mut server = Server::with_state(state);

    server.at(&cfg.graphiql_uri).get(handle_graphiql);
    server.at(&cfg.graphql_uri).post(handle_graphql);
    server.listen(cfg.listening_address).await.unwrap();
}
