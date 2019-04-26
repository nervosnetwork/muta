#![feature(async_await, await_macro, futures_api)]

use std::thread;
use std::time::Duration;

use env_logger;
use futures::sync::mpsc::channel;
use log::info;

use core_context::Context;
use core_network::reactor::inbound::LoggerInboundReactor;
use core_network::{Config, Message, Network};
use core_types::SignedTransaction;

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    env_logger::init();

    let ctx = Context::new();
    let mut config = Config::default();

    if std::env::args().nth(1) == Some("server".to_string()) {
        info!("Starting server .......");
        config.p2p.listening_address = Some("/ip4/127.0.0.1/tcp/1337".to_owned());
    } else {
        info!("Starting client ......");
        let port = std::env::args().nth(1).unwrap().parse::<u64>().unwrap();
        config.p2p.bootstrap_addresses = vec!["/ip4/127.0.0.1/tcp/1337".to_owned()];
        config.p2p.listening_address = Some(format!("/ip4/127.0.0.1/tcp/{}", port));
    }

    let (_tx, rx) = channel(10);
    let reactor = LoggerInboundReactor;
    let mut network = Network::new(config, rx, reactor).unwrap();

    for _ in 1..=4 {
        let mut stx = SignedTransaction::default();
        stx.untx.signature = b"hello world".to_vec();
        network.send(ctx.clone(), Message::BroadcastTxs { txs: vec![stx] });
    }
    thread::sleep(Duration::from_secs(5));
    await!(network.shutdown());
}
