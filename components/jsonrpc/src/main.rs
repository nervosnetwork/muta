#![feature(async_await, await_macro, futures_api)]

#[allow(dead_code)] // TODO: remove the flag
mod convention;
mod error;
mod server;

fn main() {
    let _ = server::start_server();
}
