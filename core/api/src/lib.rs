pub mod adapter;
pub mod config;
mod schema;

use std::cmp;
use std::convert::TryFrom;
use std::sync::Arc;
use std::time::Instant;

use actix_web::{web, App, Error, FromRequest, HttpResponse, HttpServer};
use futures::executor::block_on;
use juniper::http::GraphQLRequest;
use juniper::FieldResult;
use lazy_static::lazy_static;
use serde_json::json;
use std::cmp;
use std::convert::TryFrom;
use std::sync::Arc;

use common_apm::muta_apm;
use common_crypto::{
    HashValue, PrivateKey, PublicKey, Secp256k1PrivateKey, Signature, ToPublicKey,
};

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{APIAdapter, Context};
use protocol::types::Hex;

use crate::config::GraphQLConfig;
use crate::schema::{
    to_signed_transaction, to_transaction, Address, Block, Bytes, Hash, InputRawTransaction,
    InputTransactionEncryption, Receipt, ServiceResponse, SignedTransaction, Uint64,
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
    async fn get_block(state_ctx: &State, height: Option<Uint64>) -> FieldResult<Block> {
        let ctx = Context::new();
        let ctx = match muta_apm::MUTA_TRACER.span("API.getBlock", vec![
            muta_apm::rustracing::tag::Tag::new("kind", "API"),
        ]) {
            Some(span) => ctx.with_value("parent_span_ctx", span.context().cloned()),
            None => ctx,
        };

        let inst = Instant::now();
        common_apm::metrics::api::API_REQUEST_COUNTER_VEC_STATIC
            .get_block
            .inc();

        let height = match height {
            Some(id) => match id.try_into_u64() {
                Ok(id) => Some(id),
                Err(err) => {
                    common_apm::metrics::api::API_REQUEST_RESULT_COUNTER_VEC_STATIC
                        .get_block
                        .failure
                        .inc();

                    return Err(err.into());
                }
            },
            None => None,
        };

        let block = match state_ctx
            .adapter
            .get_block_by_height(ctx.clone(), height)
            .await
        {
            Ok(block) => block,
            Err(err) => {
                common_apm::metrics::api::API_REQUEST_RESULT_COUNTER_VEC_STATIC
                    .get_block
                    .failure
                    .inc();

                return Err(err.into());
            }
        };

        common_apm::metrics::api::API_REQUEST_RESULT_COUNTER_VEC_STATIC
            .get_block
            .success
            .inc();
        common_apm::metrics::api::API_REQUEST_TIME_HISTOGRAM_STATIC
            .get_block
            .observe(common_apm::metrics::duration_to_sec(inst.elapsed()));

        Ok(Block::from(block))
    }

    #[graphql(name = "getTransaction", description = "Get the transaction by hash")]
    async fn get_transaction(state_ctx: &State, tx_hash: Hash) -> FieldResult<SignedTransaction> {
        let ctx = Context::new();
        let ctx = match muta_apm::MUTA_TRACER.span("API.get_transaction", vec![
            muta_apm::rustracing::tag::Tag::new("kind", "API"),
        ]) {
            Some(span) => ctx.with_value("parent_span_ctx", span.context().cloned()),
            None => ctx,
        };

        let hash = protocol::types::Hash::from_hex(&tx_hash.as_hex())?;

        let stx = state_ctx
            .adapter
            .get_transaction_by_hash(ctx.clone(), hash)
            .await?;

        Ok(SignedTransaction::from(stx))
    }

    #[graphql(
        name = "getReceipt",
        description = "Get the receipt by transaction hash"
    )]
    async fn get_receipt(state_ctx: &State, tx_hash: Hash) -> FieldResult<Receipt> {
        let ctx = Context::new();
        let ctx = match muta_apm::MUTA_TRACER.span("API.get_receipt", vec![
            muta_apm::rustracing::tag::Tag::new("kind", "API"),
        ]) {
            Some(span) => ctx.with_value("parent_span_ctx", span.context().cloned()),
            None => ctx,
        };

        let hash = protocol::types::Hash::from_hex(&tx_hash.as_hex())?;

        let receipt = state_ctx
            .adapter
            .get_receipt_by_tx_hash(ctx.clone(), hash)
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
    ) -> FieldResult<ServiceResponse> {
        let ctx = Context::new();
        let ctx = match muta_apm::MUTA_TRACER.span("API.query_service", vec![
            muta_apm::rustracing::tag::Tag::new("kind", "API"),
        ]) {
            Some(span) => ctx.with_value("parent_span_ctx", span.context().cloned()),
            None => ctx,
        };

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
                ctx.clone(),
                height,
                cycles_limit,
                cycles_price,
                address,
                service_name,
                method,
                payload,
            )
            .await?;
        Ok(ServiceResponse::from(exec_resp))
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
        let ctx = Context::new();
        let ctx = match muta_apm::MUTA_TRACER.span("API.send_transaction", vec![
            muta_apm::rustracing::tag::Tag::new("kind", "API"),
        ]) {
            Some(span) => ctx.with_value("parent_span_ctx", span.context().cloned()),
            None => ctx,
        };

        let inst = Instant::now();
        common_apm::metrics::api::API_REQUEST_COUNTER_VEC_STATIC
            .send_transaction
            .inc();

        let stx = to_signed_transaction(input_raw, input_encryption)?;
        let tx_hash = stx.tx_hash.clone();

        if let Err(err) = state_ctx.adapter.insert_signed_txs(ctx.clone(), stx).await {
            common_apm::metrics::api::API_REQUEST_RESULT_COUNTER_VEC_STATIC
                .send_transaction
                .failure
                .inc();
            return Err(err.into());
        }

        common_apm::metrics::api::API_REQUEST_RESULT_COUNTER_VEC_STATIC
            .send_transaction
            .success
            .inc();
        common_apm::metrics::api::API_REQUEST_TIME_HISTOGRAM_STATIC
            .send_transaction
            .observe(common_apm::metrics::duration_to_sec(inst.elapsed()));

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
        let ctx = Context::new();
        let ctx = match muta_apm::MUTA_TRACER.span("API.unsafe_send_transaction", vec![
            muta_apm::rustracing::tag::Tag::new("kind", "API"),
        ]) {
            Some(span) => ctx.with_value("parent_span_ctx", span.context().cloned()),
            None => ctx,
        };

        let raw_tx = to_transaction(input_raw)?;
        let tx_hash = protocol::types::Hash::digest(raw_tx.encode_fixed()?);

        let privkey = Secp256k1PrivateKey::try_from(input_privkey.to_vec()?.as_ref())?;
        let pubkey = Hex::from_bytes(privkey.pub_key().to_bytes());

        let hash_value = HashValue::try_from(tx_hash.as_bytes().as_ref())?;
        let signature = Hex::from_bytes(privkey.sign_message(&hash_value).to_bytes());

        let wit = json!({
                    "pubkeys": [pubkey.as_string()],
                    "signatures": [signature.as_string()],
                    "signature_type": 0,
                    "sender": "0x0000000000000000000000000000000000000000",
        });

        let stx = protocol::types::SignedTransaction {
            raw:     raw_tx,
            tx_hash: tx_hash.clone(),
            witness: protocol::Bytes::from(wit.to_string()),
            sender:  None,
        };
        state_ctx
            .adapter
            .insert_signed_txs(ctx.clone(), stx)
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

async fn metrics() -> HttpResponse {
    let metrics_data = match common_apm::metrics::all_metrics() {
        Ok(data) => data,
        Err(e) => e.to_string().into_bytes(),
    };

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(metrics_data)
}

pub async fn start_graphql<Adapter: APIAdapter + 'static>(cfg: GraphQLConfig, adapter: Adapter) {
    let schema = Schema::new(Query, Mutation);

    let state = State {
        adapter: Arc::new(Box::new(adapter)),
        schema:  Arc::new(schema),
    };

    let path_graphql_uri = cfg.graphql_uri.to_owned();
    let path_graphiql_uri = cfg.graphiql_uri.to_owned();
    let workers = cfg.workers;
    let maxconn = cfg.maxconn;
    let add_listening_address = cfg.listening_address;
    let max_payload_size = cfg.max_payload_size;

    // Start http server
    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .service(
                web::resource(&path_graphql_uri)
                    .app_data(web::Json::<GraphQLRequest>::configure(|cfg| {
                        cfg.limit(max_payload_size)
                    }))
                    .route(web::post().to(graphql)),
            )
            .service(web::resource(&path_graphiql_uri).route(web::get().to(graphiql)))
            .service(web::resource("/metrics").route(web::get().to(metrics)))
    })
    .workers(workers)
    .maxconn(cmp::max(maxconn / workers, 1))
    .bind(add_listening_address)
    .unwrap()
    .run()
    .await
    .unwrap()
}
