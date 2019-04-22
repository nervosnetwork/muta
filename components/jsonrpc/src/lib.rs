#![feature(async_await, await_macro, futures_api)]

mod config;
mod convention;
mod error;
mod server;
mod state;
mod types;
mod util;

pub use config::Config;
pub use server::listen;
pub use state::AppState;

pub type RpcResult<T> = Result<T, error::RpcError>;
