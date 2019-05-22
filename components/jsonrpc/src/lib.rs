#![feature(async_await)]

#[allow(clippy::all)] // cita.rs copies from cita-common, just omit all checks
mod cita;
mod config;
mod convention;
mod error;
mod filter;
mod server;
mod state;
mod util;

pub use config::Config;
pub use server::listen;
pub use state::AppState;

pub type RpcResult<T> = Result<T, error::RpcError>;
