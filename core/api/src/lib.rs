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
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

use common_crypto::{
    HashValue, PrivateKey, PublicKey, Secp256k1PrivateKey, Signature, ToPublicKey,
};

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{APIAdapter, Context};

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
    async fn get_block(state_ctx: &State, height: Option<Uint64>) -> FieldResult<Option<Block>> {
        let ctx = Context::new();
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

        let opt_block = match state_ctx
            .adapter
            .get_block_by_height(ctx.clone(), height)
            .await
        {
            Ok(opt_block) => opt_block,
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

        Ok(opt_block.map(Block::from))
    }

    #[graphql(name = "getTransaction", description = "Get the transaction by hash")]
    async fn get_transaction(
        state_ctx: &State,
        tx_hash: Hash,
    ) -> FieldResult<Option<SignedTransaction>> {
        let ctx = Context::new();

        let hash = protocol::types::Hash::from_hex(&tx_hash.as_hex())?;

        let opt_stx = state_ctx
            .adapter
            .get_transaction_by_hash(ctx.clone(), hash)
            .await?;

        Ok(opt_stx.map(SignedTransaction::from))
    }

    #[graphql(
        name = "getReceipt",
        description = "Get the receipt by transaction hash"
    )]
    async fn get_receipt(state_ctx: &State, tx_hash: Hash) -> FieldResult<Option<Receipt>> {
        let ctx = Context::new();

        let hash = protocol::types::Hash::from_hex(&tx_hash.as_hex())?;

        let opt_receipt = state_ctx
            .adapter
            .get_receipt_by_tx_hash(ctx.clone(), hash)
            .await?;

        Ok(opt_receipt.map(Receipt::from))
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

        let height = match height {
            Some(id) => id.try_into_u64()?,
            None => {
                block_on(state_ctx.adapter.get_block_by_height(Context::new(), None))?
                    .expect("Always not none")
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

        let address: protocol::types::Address = caller.to_str().parse()?;

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

mod profile {
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::time::Duration;

    use actix_web::error::{ErrorBadRequest, ErrorInternalServerError};
    use actix_web::{dev, FromRequest, HttpRequest, HttpResponse};
    use futures::future;
    use pprof::protos::Message;

    pub enum ProfileReport {
        /// Perf flamegraph
        FlameGraph,
        /// Go pprof
        PProf,
    }

    impl FromStr for ProfileReport {
        type Err = &'static str;

        fn from_str(report: &str) -> Result<Self, Self::Err> {
            match report {
                "flamegraph" => Ok(ProfileReport::FlameGraph),
                "pprof" => Ok(ProfileReport::PProf),
                _ => Err("invalid report type, only support flamegraph and pprof"),
            }
        }
    }

    pub struct ProfileConfig {
        duration:  Duration,
        frequency: i32,
        report:    ProfileReport,
    }

    impl Default for ProfileConfig {
        fn default() -> Self {
            ProfileConfig {
                duration:  Duration::from_secs(10),
                frequency: 99,
                report:    ProfileReport::FlameGraph,
            }
        }
    }

    impl FromRequest for ProfileConfig {
        type Config = ();
        type Error = actix_web::Error;
        type Future = future::Ready<Result<Self, Self::Error>>;

        fn from_request(req: &HttpRequest, _: &mut dev::Payload) -> Self::Future {
            let query = req.query_string();
            let query_pairs: HashMap<_, _> =
                url::form_urlencoded::parse(query.as_bytes()).collect();

            let duration: Duration = match query_pairs.get("duration").map(|val| val.parse()) {
                Some(Ok(val)) => Duration::from_secs(val),
                Some(Err(e)) => return future::err(ErrorBadRequest(e)),
                None => ProfileConfig::default().duration,
            };

            let frequency: i32 = match query_pairs.get("frequency").map(|val| val.parse()) {
                Some(Ok(val)) => val,
                Some(Err(e)) => return future::err(ErrorBadRequest(e)),
                None => ProfileConfig::default().frequency,
            };

            let report: ProfileReport = match query_pairs.get("report").map(|val| val.parse()) {
                Some(Ok(val)) => val,
                Some(Err(e)) => return future::err(ErrorBadRequest(e)),
                None => ProfileConfig::default().report,
            };

            future::ok(ProfileConfig {
                duration,
                frequency,
                report,
            })
        }
    }

    pub async fn dump_profile(maybe_config: actix_web::Result<ProfileConfig>) -> HttpResponse {
        let config = match maybe_config {
            Ok(config) => config,
            Err(e) => return e.into(),
        };

        let guard = match pprof::ProfilerGuard::new(config.frequency) {
            Ok(guard) => guard,
            Err(e) => return ErrorInternalServerError(e).into(),
        };

        tokio::time::delay_for(config.duration).await;
        let report = match guard.report().build() {
            Ok(report) => report,
            Err(e) => return ErrorInternalServerError(e).into(),
        };
        drop(guard);

        let mut body = Vec::new();
        match config.report {
            ProfileReport::FlameGraph => match report.flamegraph(&mut body) {
                Ok(_) => {
                    log::info!("dump flamegraph successfully");
                    HttpResponse::Ok().body(body)
                }
                Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
            },
            ProfileReport::PProf => match report.pprof().map(|p| p.encode(&mut body)) {
                Ok(Ok(())) => {
                    log::info!("dump pprof successfully");
                    HttpResponse::Ok().body(body)
                }
                Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
                Ok(Err(err)) => HttpResponse::InternalServerError().body(err.to_string()),
            },
        }
    }
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
    let enable_dump_profile = cfg.enable_dump_profile;

    // Start http server
    let server = HttpServer::new(move || {
        let app = App::new()
            .data(state.clone())
            .service(
                web::resource(&path_graphql_uri)
                    .app_data(web::Json::<GraphQLRequest>::configure(|cfg| {
                        cfg.limit(max_payload_size)
                    }))
                    .route(web::post().to(graphql)),
            )
            .service(web::resource(&path_graphiql_uri).route(web::get().to(graphiql)))
            .service(web::resource("/metrics").route(web::get().to(metrics)));

        if enable_dump_profile {
            app.service(web::resource("/dump_profile").route(web::get().to(profile::dump_profile)))
        } else {
            app
        }
    })
    .workers(workers)
    .maxconn(cmp::max(maxconn / workers, 1));

    if let Some(tls) = cfg.tls {
        // load ssl keys
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        builder
            .set_private_key_file(tls.private_key_file_path, SslFiletype::PEM)
            .unwrap();
        builder
            .set_certificate_chain_file(tls.certificate_chain_file_path)
            .unwrap();

        server
            .bind_openssl(add_listening_address, builder)
            .unwrap()
            .run()
            .await
            .unwrap()
    } else {
        server
            .bind(add_listening_address)
            .unwrap()
            .run()
            .await
            .unwrap()
    }
}
