use actix_web::{self, middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use futures::compat::Future01CompatExt;
use futures::prelude::{FutureExt, TryFutureExt};
use old_futures::{self, Future as OldFuture};
use serde_json;
use serde_json::Value;

use core_runtime::{Executor, TransactionPool};
use core_storage::Storage;
use core_types::Address;

use crate::config::Config;
use crate::convention;
use crate::state::AppState;
use crate::util::clean_0x;

fn rpc_handle<E: 'static, T: 'static, S: 'static>(
    reqjson: web::Json<convention::Request>,
    app_state: web::Data<AppState<E, T, S>>,
    _req: HttpRequest,
) -> Box<OldFuture<Item = HttpResponse, Error = actix_web::Error>>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    let mut result = convention::Response::default();
    result.id = reqjson.id.clone();

    let fut = async move {
        match await!(rpc_select(
            app_state.get_ref().clone(),
            reqjson.method.clone(),
            reqjson.params.clone()
        )
        .compat())
        {
            Ok(ok) => result.result = ok,
            Err(e) => result.error = Some(e),
        }
        Ok(HttpResponse::Ok().json(result))
    };

    Box::new(fut.boxed().compat())
}

fn rpc_select<E: 'static, T: 'static, S: 'static>(
    app_state: AppState<E, T, S>,
    method: String,
    params: Option<Vec<Value>>,
) -> Box<OldFuture<Item = Value, Error = convention::ErrorData>>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    let fut = async move {
        match method.as_str() {
            // Get block number. CITA api needs hex string with 0x prefix, sad.
            "blockNumber" => {
                let r = await!(app_state.block_number())?;
                Ok(Value::from(format!("{:#x}", r)))
            }
            // Get balance by address and [block number].
            "getBalance" => {
                let params = params.unwrap_or_default();
                let addr_str = params[0]
                    .as_str()
                    .ok_or_else(|| convention::ErrorData::std(-32602))?;
                let addr = Address::from_hex(clean_0x(addr_str))?;
                let number = params[1].as_str().unwrap_or_default();
                let r = await!(app_state.get_balance(String::from(number), addr))?;
                Ok(Value::from(format!("{:#x}", r)))
            }
            // Test whether the server is still aliving. It's not in CITA spec.
            "ping" => Ok(Value::from("pong")),
            // Send a raw transaction to chain. Yes, indeed.
            "sendRawTransaction" => {
                let params = params.unwrap_or_default();
                let data_str = params[0]
                    .as_str()
                    .ok_or_else(|| convention::ErrorData::std(-32602))?;
                let data = hex::decode(clean_0x(data_str))?;
                let r = await!(app_state.send_raw_transaction(data))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            _ => Err(convention::ErrorData::std(-32601)),
        }
    };
    Box::new(fut.boxed().compat())
}

/// Listen and server on address:port which definds on config
pub fn listen<E: 'static, T: 'static, S: 'static>(
    config: Config,
    app_state: AppState<E, T, S>,
) -> std::io::Result<()>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    let c_payload_size = config.payload_size;
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .data(app_state.clone())
            .service(
                web::resource("/").route(
                    web::post()
                        .data(web::JsonConfig::default().limit(c_payload_size)) // <- limit size of the payload
                        .to_async(rpc_handle::<E, T, S>),
                ),
            )
    })
    .bind(config.listen)?
    .workers(config.workers)
    .run()
}
