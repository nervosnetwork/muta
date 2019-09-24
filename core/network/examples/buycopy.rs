use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    thread,
    time::Duration,
};

use async_trait::async_trait;
use bytes::Bytes;
use log::info;
use serde_derive::{Deserialize, Serialize};
use tentacle::secio::SecioKeyPair;

use core_network::{NetworkConfig, NetworkService};
use protocol::{
    traits::{Context, Gossip, MessageHandler, Priority, Rpc},
    types::Hash,
    ProtocolResult,
};

const IP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));

const RELEASE_CHANNEL: &str = "/gossip/cprd/cyperpunk7702_released";
const SHOP_CASH_CHANNEL: &str = "/rpc_call/v3/steam";
const SHOP_CHANNEL: &str = "/rpc_resp/v3/steam";

// Gossip message
#[derive(Debug, Serialize, Deserialize)]
struct Cyber7702Released {
    pub shop: String,
    #[serde(with = "core_network::serde")]
    pub hash: Hash,
}

// Gossip message handler
struct TakeMyMoney<N: Rpc> {
    pub shop: N,
}

#[async_trait]
impl<N: Rpc + Send + Sync + 'static> MessageHandler for TakeMyMoney<N> {
    type Message = Cyber7702Released;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        println!("Rush to {}. Shut up, take my money", msg.shop);

        let copy: ACopy = self
            .shop
            .call(ctx, SHOP_CASH_CHANNEL, BuyACopy, Priority::High)
            .await?;
        println!("Got my copy {:?}", copy);

        Ok(())
    }
}

// Rpc message
#[derive(Debug, Serialize, Deserialize)]
struct BuyACopy;

#[derive(Debug, Serialize, Deserialize)]
struct ACopy {
    #[serde(with = "core_network::serde")]
    pub hash: Hash,

    #[serde(with = "core_network::serde_multi")]
    pub gifs: Vec<Hash>,
}

// Rpc call message handler
struct Checkout<N: Rpc> {
    dealer: N,
}

#[async_trait]
impl<N: Rpc + Send + Sync + 'static> MessageHandler for Checkout<N> {
    type Message = BuyACopy;

    async fn process(&self, ctx: Context, _msg: Self::Message) -> ProtocolResult<()> {
        let acopy = ACopy {
            hash: Hash::digest(Bytes::new()),
            gifs: vec![
                Hash::digest("jacket".into()),
                Hash::digest("map".into()),
                Hash::digest("book".into()),
            ],
        };

        self.dealer
            .response(ctx, SHOP_CHANNEL, acopy, Priority::High)
            .await
    }
}

#[runtime::main(runtime_tokio::Tokio)]
pub async fn main() {
    env_logger::init();

    let base_conf = NetworkConfig::new();

    let bt_seckey_bytes = "8".repeat(32);
    let bt_seckey = hex::encode(&bt_seckey_bytes);
    let bt_keypair = SecioKeyPair::secp256k1_raw_key(bt_seckey_bytes).expect("keypair");
    let bt_pubkey = hex::encode(bt_keypair.to_public_key().inner());
    let bt_addr = SocketAddr::new(IP_ADDR, 1337);

    if std::env::args().nth(1) == Some("server".to_string()) {
        info!("Starting server");

        let bt_conf = base_conf
            .clone()
            .secio_keypair(bt_seckey)
            .expect("set keypair");

        let mut bootstrap = NetworkService::new(bt_conf);
        let handle = bootstrap.handle();
        bootstrap.listen(bt_addr).unwrap();

        let check_out = Checkout {
            dealer: handle.clone(),
        };
        bootstrap
            .register_endpoint_handler(SHOP_CASH_CHANNEL, Box::new(check_out))
            .unwrap();

        runtime::spawn(bootstrap);
        thread::sleep(Duration::from_secs(10));

        let released = Cyber7702Released {
            shop: "steam".to_owned(),
            hash: Hash::digest(Bytes::from("buy".repeat(3))),
        };

        let ctx = Context::default();
        handle
            .broadcast(ctx.clone(), RELEASE_CHANNEL, released, Priority::High)
            .await
            .unwrap();

        thread::sleep(Duration::from_secs(10));
    } else {
        info!("Starting client");

        let port = std::env::args().nth(1).unwrap().parse::<u16>().unwrap();
        let peer_addr = SocketAddr::new(IP_ADDR, port);
        let peer_conf = base_conf
            .clone()
            .bootstraps(vec![(bt_pubkey, bt_addr)])
            .unwrap();

        let mut peer = NetworkService::new(peer_conf);
        let handle = peer.handle();
        peer.listen(peer_addr).unwrap();

        let take_my_money = TakeMyMoney {
            shop: handle.clone(),
        };
        peer.register_endpoint_handler(RELEASE_CHANNEL, Box::new(take_my_money))
            .unwrap();
        peer.register_rpc_response::<ACopy>(SHOP_CHANNEL).unwrap();

        peer.await;
    }
}
