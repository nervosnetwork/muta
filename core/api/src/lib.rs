pub mod adapter;
pub mod config;
mod schema;

use actix_web::{web, App, Error, HttpResponse, HttpServer};
use futures::executor::block_on;
use juniper::http::GraphQLRequest;
use juniper::FieldResult;
use lazy_static::lazy_static;
use std::convert::TryFrom;
use std::sync::Arc;

use common_crypto::{
    HashValue, PrivateKey, PublicKey, Secp256k1PrivateKey, Signature, ToPublicKey,
};
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{APIAdapter, Context};

use crate::config::GraphQLConfig;
use crate::schema::{
    to_signed_transaction, to_transaction, Address, Block, Bytes, ExecResp, Hash,
    InputRawTransaction, InputTransactionEncryption, Receipt, SignedTransaction, Uint64,
};

lazy_static! {
    static ref GRAPHIQL_HTML: &'static str = include_str!("../source/graphiql.html");
}

// This is accessible as state in Tide, and as executor context in Juniper.
#[derive(Clone)]
struct State {
    adapter: Arc<Box<dyn APIAdapter>>,
    schema:  Arc<Schema>,
}

// We define `Query` unit struct here. GraphQL queries will refer to this
// struct. The struct itself doesn't have any associated state (and there's no
// need to do so), but instead it exposes the accumulator state from the
// context.
struct Query;
// Switch to async/await fn https://github.com/graphql-rust/juniper/issues/2
#[juniper::graphql_object(Context = State)]
impl Query {
    #[graphql(name = "getBlock", description = "Get the block")]
    async fn get_latest_block(state_ctx: &State, height: Option<Uint64>) -> FieldResult<Block> {
        let height = match height {
            Some(id) => Some(id.try_into_u64()?),
            None => None,
        };

        let block = state_ctx
            .adapter
            .get_block_by_height(Context::new(), height)
            .await?;

        Ok(Block::from(block))
    }

    #[graphql(name = "getTransaction", description = "Get the transaction by hash")]
    async fn get_transaction(state_ctx: &State, tx_hash: Hash) -> FieldResult<SignedTransaction> {
        let hash = protocol::types::Hash::from_hex(&tx_hash.as_hex())?;

        let stx = state_ctx
            .adapter
            .get_transaction_by_hash(Context::new(), hash)
            .await?;

        Ok(SignedTransaction::from(stx))
    }

    #[graphql(
        name = "getReceipt",
        description = "Get the receipt by transaction hash"
    )]
    async fn get_receipt(state_ctx: &State, tx_hash: Hash) -> FieldResult<Receipt> {
        let hash = protocol::types::Hash::from_hex(&tx_hash.as_hex())?;

        let receipt = state_ctx
            .adapter
            .get_receipt_by_tx_hash(Context::new(), hash)
            .await?;

        Ok(Receipt::from(receipt))
    }

    #[graphql(name = "queryService", description = "query service")]
    async fn query_service(
        state_ctx: &State,
        height: Option<Uint64>,
        cycles_limit: Option<Uint64>,
        cycles_price: Option<Uint64>,
        caller: Address,
        service_name: String,
        method: String,
        payload: String,
    ) -> FieldResult<ExecResp> {
        let height = match height {
            Some(id) => id.try_into_u64()?,
            None => {
                block_on(state_ctx.adapter.get_block_by_height(Context::new(), None))?
                    .header
                    .height
            }
        };
        let cycles_limit = match cycles_limit {
            Some(cycles_limit) => cycles_limit.try_into_u64()?,
            None => std::u64::MAX,
        };

        let cycles_price = match cycles_price {
            Some(cycles_price) => cycles_price.try_into_u64()?,
            None => 1,
        };

        let address = protocol::types::Address::from_hex(&caller.as_hex())?;

        let exec_resp = state_ctx
            .adapter
            .query_service(
                Context::new(),
                height,
                cycles_limit,
                cycles_price,
                address,
                service_name,
                method,
                payload,
            )
            .await?;
        Ok(ExecResp::from(exec_resp))
    }
}

struct Mutation;
// Switch to async/await fn https://github.com/graphql-rust/juniper/issues/2
#[juniper::graphql_object(Context = State)]
impl Mutation {
    #[graphql(name = "sendTransaction", description = "send transaction")]
    async fn send_transaction(
        state_ctx: &State,
        input_raw: InputRawTransaction,
        input_encryption: InputTransactionEncryption,
    ) -> FieldResult<Hash> {
        let stx = to_signed_transaction(input_raw, input_encryption)?;
        let tx_hash = stx.tx_hash.clone();

        state_ctx
            .adapter
            .insert_signed_txs(Context::new(), stx)
            .await?;

        Ok(Hash::from(tx_hash))
    }

    #[graphql(
        name = "unsafeSendTransaction",
        deprecated = "DON'T use it in production! This is just for development."
    )]
    async fn unsafe_send_transaction(
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
        state_ctx
            .adapter
            .insert_signed_txs(Context::new(), stx)
            .await?;

        Ok(Hash::from(tx_hash))
    }
}

// Adding `Query` and `Mutation` together we get `Schema`, which describes,
// well, the whole GraphQL schema.
type Schema = juniper::RootNode<'static, Query, Mutation>;

async fn graphiql() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GRAPHIQL_HTML.to_owned())
}

async fn graphql(
    st: web::Data<State>,
    data: web::Json<GraphQLRequest>,
) -> Result<HttpResponse, Error> {
    let result = data.execute_async(&st.schema, &st).await;
    let res = Ok::<_, serde_json::error::Error>(serde_json::to_string(&result)?)?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(res))
}

pub async fn start_graphql<Adapter: APIAdapter + 'static>(cfg: GraphQLConfig, adapter: Adapter) {
    let schema = Schema::new(Query, Mutation);

    let state = State {
        adapter: Arc::new(Box::new(adapter)),
        schema:  Arc::new(schema),
    };

    let path_graphql_uri = cfg.graphql_uri.to_owned();
    let path_graphiql_uri = cfg.graphiql_uri.to_owned();
    let wokers = cfg.workers;
    let maxconn = cfg.maxconn;
    let add_listening_address = cfg.listening_address;

    // Start http server
    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .service(web::resource(&path_graphql_uri).route(web::post().to(graphql)))
            .service(web::resource(&path_graphiql_uri).route(web::get().to(graphiql)))
    })
    .workers(wokers)
    .maxconn(maxconn)
    .bind(add_listening_address)
    .unwrap()
    .run()
    .await
    .unwrap()
}
