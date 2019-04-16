use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use actix_web::{self, middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use futures::compat;
use futures_timer::Delay;
use old_futures::{self, Future as OldFuture};
use serde_json;
use serde_json::Value;
use tokio_async_await::compat::{backward::Compat, forward::IntoAwaitable};

use crate::convention;
use crate::error::RpcError;

fn rpc_handle(
    reqjson: web::Json<convention::Request>,
    app_state: web::Data<AppState>,
    _req: HttpRequest,
) -> Box<OldFuture<Item = HttpResponse, Error = actix_web::Error>> {
    let mut result = convention::Response::default();
    result.id = reqjson.id.clone();

    let fut = async move {
        match await!(rpc_select(
            app_state.get_ref().clone(),
            reqjson.method.clone(),
            reqjson.params.clone()
        )
        .into_awaitable())
        {
            Ok(ok) => result.result = ok,
            Err(e) => result.error = Some(e),
        }
        Ok(HttpResponse::Ok().json(result))
    };

    Box::new(Compat::new(fut))
}

fn rpc_select(
    app_state: AppState,
    method: String,
    params: Vec<Value>,
) -> Box<OldFuture<Item = Value, Error = convention::ErrorData>> {
    let fut = async move {
        match method.as_str() {
            "ping" => {
                let r = app_state.network.read().unwrap().ping();
                Ok(Value::from(r))
            }
            "wait" => {
                if params.len() != 1 || !params[0].is_u64() {
                    return Err(convention::ErrorData::std(-32602));
                }
                let n = app_state.network.read().unwrap();
                let r = await!(n.wait(params[0].as_u64().unwrap()));
                match r {
                    Ok(ok) => Ok(Value::from(ok)),
                    Err(e) => Err(convention::ErrorData::new(500, &format!("{:?}", e)[..])),
                }
            }
            "get" => {
                let r = app_state.network.read().unwrap().get();
                Ok(Value::from(r))
            }
            "inc" => {
                app_state.network.write().unwrap().inc();
                Ok(Value::Null)
            }
            _ => Err(convention::ErrorData::std(-32601)),
        }
    };
    Box::new(Compat::new(fut))
}

pub struct ObjNetwork {
    c: u32,
}

impl ObjNetwork {
    fn new() -> Self {
        Self { c: 0 }
    }
}

impl ObjNetwork {
    fn ping(&self) -> String {
        String::from("pong")
    }

    async fn wait(&self, d: u64) -> Result<String, RpcError> {
        if let Err(e) = await!(compat::Compat01As03::new(Delay::new(Duration::from_secs(
            d
        )))) {
            return Err(e.into());
        }
        Ok(String::from("pong"))
    }

    fn get(&self) -> u32 {
        self.c
    }

    fn inc(&mut self) {
        self.c += 1;
    }
}

#[derive(Clone)]
pub struct AppState {
    network: Arc<RwLock<ObjNetwork>>,
}

impl AppState {
    pub fn new(network: Arc<RwLock<ObjNetwork>>) -> Self {
        Self { network }
    }
}

pub fn start_server() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let app_data_network = Arc::new(RwLock::new(ObjNetwork::new()));
    HttpServer::new(move || {
        let app_data = AppState::new(Arc::<RwLock<ObjNetwork>>::clone(&app_data_network));
        App::new()
            .wrap(middleware::Logger::default())
            .data(app_data)
            .service(
                web::resource("/").route(
                    web::post()
                        .data(web::JsonConfig::default().limit(4096)) // <- limit size of the payload
                        .to_async(rpc_handle),
                ),
            )
    })
    .bind("127.0.0.1:8080")?
    .workers(1)
    .run()
}
