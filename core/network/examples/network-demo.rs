#![feature(async_await, await_macro, futures_api)]

use core_network::{Config, Message, Network};

use env_logger;
use log::info;

use std::thread;
use std::time::Duration;

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    env_logger::init();

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

    let mut network = Network::new(config).unwrap();

    for _ in 1..10 {
        network.send(Message::Consensus(b"hello world".to_vec()));
    }
    thread::sleep(Duration::from_secs(10));
    await!(network.shutdown());
}
