use actix_web::middleware::cors::Cors;
use actix_web::{self, middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use futures::compat::Future01CompatExt;
use futures::prelude::{FutureExt, StreamExt, TryFutureExt};
use old_futures::{self, Future as OldFuture};
use serde_json;
use serde_json::Value;

use core_pubsub::channel::pubsub::Receiver;
use core_runtime::{Database, Executor, TransactionPool};
use core_storage::{Storage, StorageError};
use core_types::{Address, Block, Hash};

use crate::cita;
use crate::config::Config;
use crate::convention;
use crate::error::RpcError;
use crate::filter::Filter;
use crate::state::AppState;
use crate::util::clean_0x;

fn rpc_handle<E: 'static, T: 'static, S: 'static, D: 'static>(
    reqjson: web::Json<convention::Call>,
    app_state: web::Data<AppState<E, T, S, D>>,
    _req: HttpRequest,
) -> Box<OldFuture<Item = HttpResponse, Error = actix_web::Error>>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    D: Database,
{
    match reqjson.into_inner() {
        convention::Call::Single(req) => {
            let fut = async move {
                let result = await!(handle_one_request(req, app_state));
                Ok(HttpResponse::Ok().json(result))
            };
            Box::new(fut.boxed().compat())
        }
        convention::Call::Batch(reqs) => {
            let mut results = vec![];
            let fut = async move {
                for req in reqs {
                    let result = await!(handle_one_request(req, app_state.clone()));
                    results.push(result);
                }
                Ok(HttpResponse::Ok().json(results))
            };

            Box::new(fut.boxed().compat())
        }
    }
}

async fn handle_one_request<E: 'static, T: 'static, S: 'static, D: 'static>(
    req: convention::Request,
    app_state: web::Data<AppState<E, T, S, D>>,
) -> convention::Response
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    D: Database,
{
    let mut result = convention::Response::default();
    result.id = req.id.clone();

    match await!(rpc_select(
        app_state.get_ref().clone(),
        req.method.clone(),
        req.params.clone()
    )
    .compat())
    {
        Ok(ok) => result.result = Some(ok),
        Err(e) => result.error = Some(e),
    }
    result
}

fn get_string(
    params: Vec<Value>,
    index: usize,
    mustfit: bool,
) -> Result<String, convention::ErrorData> {
    let r = params.get(index);
    let r = if mustfit {
        r.ok_or_else(|| convention::ErrorData::std(-32602))?
    } else {
        r.unwrap_or(&Value::Null)
    };
    let r = r.as_str();
    let r = if mustfit {
        r.ok_or_else(|| convention::ErrorData::std(-32602))?
    } else {
        r.unwrap_or_default()
    };
    Ok(String::from(r))
}

#[allow(clippy::cognitive_complexity)]
fn rpc_select<E: 'static, T: 'static, S: 'static, D: 'static>(
    app_state: AppState<E, T, S, D>,
    method: String,
    params: Option<Vec<Value>>,
) -> Box<OldFuture<Item = Value, Error = convention::ErrorData>>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    D: Database,
{
    let fut = async move {
        let params = params.unwrap_or_default();
        match method.as_str() {
            // Get block number. CITA api needs hex string with 0x prefix.
            "blockNumber" => {
                let r = await!(app_state.block_number())?;
                Ok(Value::from(format!("{:#x}", r)))
            }
            // Call contract in readonly mode.
            "call" => {
                let req: cita::CallRequest = serde_json::from_value(
                    params
                        .get(0)
                        .ok_or_else(|| convention::ErrorData::std(-32602))?
                        .clone(),
                )?;
                let number = get_string(params, 1, false)?;
                let r = await!(app_state.call(number, req))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            // Get Abi
            "getAbi" => {
                let addr_str = get_string(params.clone(), 0, true)?;
                let addr = Address::from_hex(clean_0x(&addr_str[..]))?;
                let number = get_string(params, 1, false)?;
                let r = await!(app_state.get_abi(addr, number))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            // Get balance by [address, block number].
            "getBalance" => {
                let addr_str = get_string(params.clone(), 0, true)?;
                let addr = Address::from_hex(clean_0x(&addr_str[..]))?;
                let number = get_string(params, 1, false)?;
                let r = await!(app_state.get_balance(number, addr))?;
                Ok(Value::from(format!("{:#x}", r)))
            }
            // Get Block by [hash, include_tx]
            "getBlockByHash" => {
                let hash_str = get_string(params.clone(), 0, true)?;
                let hash = Hash::from_hex(clean_0x(&hash_str[..]))?;
                let include_tx = params
                    .get(1)
                    .ok_or_else(|| convention::ErrorData::std(-32602))?
                    .as_bool()
                    .unwrap_or_default();
                let r = await!(app_state.get_block_by_hash(hash, include_tx))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            // Get Block by [number, include_tx]
            "getBlockByNumber" => {
                let number = get_string(params.clone(), 0, false)?;
                let include_tx = params
                    .get(1)
                    .ok_or_else(|| convention::ErrorData::std(-32602))?
                    .as_bool()
                    .unwrap_or_default();
                let r = await!(app_state.get_block_by_number(number, include_tx))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            // Get BlockHeader by [number]
            "getBlockHeader" => {
                let number = get_string(params, 0, false)?;
                let r = await!(app_state.get_block_header(number))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            // Get Code
            "getCode" => {
                let addr_str = get_string(params.clone(), 0, true)?;
                let addr = Address::from_hex(clean_0x(&addr_str[..]))?;
                let number = get_string(params, 1, false)?;
                match await!(app_state.get_code(addr, number)) {
                    Ok(ok) => Ok(serde_json::to_value(ok).unwrap()),
                    Err(e) => Err(convention::ErrorData::from(e)),
                }
            }
            // Get Filter logs by filter id
            "getFilterChanges" => {
                let id = get_string(params, 0, true)?;
                let id_u64 = u64::from_str_radix(clean_0x(&id[..]), 16)?;
                let r = await!(app_state.filterdb.write().compat())
                    .unwrap()
                    .filter_changes(id_u64 as u32);
                Ok(serde_json::to_value(r).unwrap())
            }
            // Get logs by filter
            "getLogs" => {
                let filter: cita::Filter = serde_json::from_value(
                    params
                        .get(0)
                        .ok_or_else(|| convention::ErrorData::std(-32602))?
                        .clone(),
                )?;
                let r = await!(app_state.get_logs(filter))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            // Get Metadata of this chain
            "getMetaData" => {
                let number = get_string(params, 0, false)?;
                let r = await!(app_state.get_metadata(number))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            // Get proof of state by [address, key_hash, block_number]
            "getStateProof" => {
                let addr_str = get_string(params.clone(), 0, true)?;
                let addr = Address::from_hex(clean_0x(&addr_str[..]))?;
                let hash_str = get_string(params.clone(), 1, true)?;
                let hash = Hash::from_hex(clean_0x(&hash_str[..]))?;
                let number = get_string(params, 2, false)?;
                match await!(app_state.get_state_proof(addr, hash, number)) {
                    Ok(ok) => Ok(serde_json::to_value(ok).unwrap()),
                    Err(RpcError::StorageError(StorageError::None(_))) => Ok(Value::Null),
                    Err(e) => Err(convention::ErrorData::from(e)),
                }
            }
            // Get value of key in storage by [address, key_hash, block_number]
            "getStorageAt" => {
                let addr_str = get_string(params.clone(), 0, true)?;
                let addr = Address::from_hex(clean_0x(&addr_str[..]))?;
                let hash_str = get_string(params.clone(), 1, true)?;
                let hash = Hash::from_hex(clean_0x(&hash_str[..]))?;
                let number = get_string(params, 2, false)?;
                match await!(app_state.get_storage_at(addr, hash.as_fixed_bytes().into(), number)) {
                    Ok(ok) => Ok(serde_json::to_value(ok).unwrap()),
                    Err(RpcError::StorageError(StorageError::None(_))) => Ok(Value::Null),
                    Err(e) => Err(convention::ErrorData::from(e)),
                }
            }
            // Get transaction by hash
            "getTransaction" => {
                let hash_str = get_string(params, 0, true)?;
                let hash = Hash::from_hex(clean_0x(&hash_str[..]))?;
                match await!(app_state.get_transaction(hash)) {
                    Ok(ok) => Ok(serde_json::to_value(ok).unwrap()),
                    Err(RpcError::StorageError(StorageError::None(_))) => Ok(Value::Null),
                    Err(e) => Err(convention::ErrorData::from(e)),
                }
            }
            // Get the nonce of address
            "getTransactionCount" => {
                let addr_str = get_string(params.clone(), 0, true)?;
                let addr = Address::from_hex(clean_0x(&addr_str[..]))?;
                let number = get_string(params, 1, false)?;
                let r = await!(app_state.get_transaction_count(addr, number))?;
                Ok(Value::from(format!("{:#x}", r)))
            }
            // Get the proof of transaction by [tx_hash]
            "getTransactionProof" => {
                let hash_str = get_string(params, 0, true)?;
                let hash = Hash::from_hex(clean_0x(&hash_str[..]))?;
                match await!(app_state.get_transaction_proof(hash)) {
                    Ok(ok) => Ok(serde_json::to_value(ok).unwrap()),
                    Err(RpcError::StorageError(StorageError::None(_))) => Ok(Value::Null),
                    Err(e) => Err(convention::ErrorData::from(e)),
                }
            }
            // Get receipt by transaction's hash
            "getTransactionReceipt" => {
                let hash_str = get_string(params, 0, true)?;
                let hash = Hash::from_hex(clean_0x(&hash_str[..]))?;
                match await!(app_state.get_transaction_receipt(hash)) {
                    Ok(ok) => Ok(serde_json::to_value(ok).unwrap()),
                    Err(RpcError::StorageError(StorageError::None(_))) => Ok(Value::Null),
                    Err(e) => Err(convention::ErrorData::from(e)),
                }
            }
            // Register a new block filter
            "newBlockFilter" => {
                let r = await!(app_state.filterdb.write().compat())
                    .unwrap()
                    .new_block_filter(0);
                Ok(Value::from(format!("{:#x}", r)))
            }
            // Register a new filter
            "newFilter" => {
                let filter: cita::Filter = serde_json::from_value(
                    params
                        .get(0)
                        .ok_or_else(|| convention::ErrorData::std(-32602))?
                        .clone(),
                )?;
                let filter: Filter = filter.into();
                let r = await!(app_state.filterdb.write().compat())
                    .unwrap()
                    .new_filter(filter);
                Ok(Value::from(format!("{:#x}", r)))
            }
            // Get the count of peers
            "peerCount" => {
                let r = await!(app_state.peer_count())?;
                Ok(Value::from(r))
            }
            // Test whether the server is still aliving. It's not in CITA spec.
            "ping" => Ok(Value::from("pong")),
            // Send a raw transaction to chain. Yes, indeed.
            "sendTransaction" | "sendRawTransaction" => {
                let data_str = get_string(params, 0, true)?;
                let data = hex::decode(clean_0x(&data_str[..]))?;
                let r = await!(app_state.send_raw_transaction(data))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            "uninstallFilter" => {
                let id = get_string(params, 0, true)?;
                let id_u64 = u64::from_str_radix(clean_0x(&id[..]), 16)?;
                let r = await!(app_state.filterdb.write().compat())
                    .unwrap()
                    .uninstall(id_u64 as u32);
                Ok(serde_json::to_value(r).unwrap())
            }
            _ => Err(convention::ErrorData::std(-32601)),
        }
    };
    Box::new(fut.boxed().compat())
}

/// Listen and server on address:port which definds on config
pub fn listen<E: 'static, T: 'static, S: 'static, D: 'static>(
    config: Config,
    app_state: AppState<E, T, S, D>,
    mut sub_block: Receiver<Block>,
) -> std::io::Result<()>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    D: Database,
{
    let mut app_state_clone = app_state.clone();
    let fut = async move {
        let mut s: &mut Receiver<Block> = &mut sub_block;
        while let Some(e) = await!(s.next()) {
            if let Some(b) = e {
                if let Err(e) = await!(app_state_clone.recv_block(b)) {
                    println!("{:?}", e);
                };
            }
        }
    };
    std::thread::spawn(move || {
        futures::executor::block_on(fut);
    });

    let c_payload_size = config.payload_size;
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(Cors::default())
            .data(app_state.clone())
            .service(
                web::resource("/").route(
                    web::post()
                        .data(web::JsonConfig::default().limit(c_payload_size)) // <- limit size of the payload
                        .to_async(rpc_handle::<E, T, S, D>),
                ),
            )
    })
    .bind(config.listen)?
    .workers(config.workers)
    .run()
}
