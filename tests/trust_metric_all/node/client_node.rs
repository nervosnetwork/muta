use super::{config::Config, consts};

use common_crypto::{PrivateKey, Secp256k1PrivateKey};
use core_consensus::message::{
    FixedBlock, FixedHeight, RPC_RESP_SYNC_PULL_BLOCK, RPC_SYNC_PULL_BLOCK,
};
use core_network::{NetworkConfig, NetworkService, NetworkServiceHandle};
use protocol::{
    async_trait,
    traits::{Context, Gossip, MessageCodec, Priority, Rpc},
    types::{Address, Block},
    ProtocolResult,
};

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[async_trait]
pub trait ClientNodeRPC {
    async fn genesis_block(&self) -> ProtocolResult<Block>;
    async fn connected(&self) -> bool;
    async fn broadcast<M: MessageCodec>(&self, end: &str, msg: M) -> ProtocolResult<()>;
}

pub struct ClientNode {
    pub network:           NetworkServiceHandle,
    pub remote_chain_addr: Address,
    pub priv_key:          Secp256k1PrivateKey,
}

pub async fn connect(full_node_port: u16, listen_port: u16) -> ClientNode {
    let full_node_hex_pubkey = full_node_hex_pubkey();
    let full_node_chain_addr = full_node_chain_addr(&full_node_hex_pubkey);
    let full_node_addr = format!("127.0.0.1:{}", full_node_port);

    let config = NetworkConfig::new()
        .ping_interval(consts::NETWORK_PING_INTERVAL)
        .peer_trust_metric(consts::NETWORK_TRUST_METRIC_INTERVAL, None)
        .expect("peer trust")
        .bootstraps(vec![(full_node_hex_pubkey, full_node_addr)])
        .expect("test node config");
    let priv_key = Secp256k1PrivateKey::generate(&mut rand::rngs::OsRng);

    let mut network = NetworkService::new(config);
    let handle = network.handle();

    network
        .register_rpc_response::<FixedBlock>(RPC_RESP_SYNC_PULL_BLOCK)
        .expect("register consensus rpc pull block");

    network
        .listen(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            listen_port,
        ))
        .await
        .expect("test node listen");

    tokio::spawn(network);

    let mut count = 100u8;
    while count > 0 {
        count -= 1;
        if handle
            .diagnostic
            .session_by_chain(&full_node_chain_addr)
            .is_some()
        {
            break;
        }
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
    }
    if count == 0 {
        panic!("failed to connect full node");
    }

    ClientNode {
        network: handle,
        remote_chain_addr: full_node_chain_addr,
        priv_key,
    }
}

#[async_trait]
impl ClientNodeRPC for ClientNode {
    async fn genesis_block(&self) -> ProtocolResult<Block> {
        let ctx = Context::new().with_value::<usize>("session_id", 1);
        let fixed_block = self
            .network
            .call::<FixedHeight, FixedBlock>(
                ctx,
                RPC_SYNC_PULL_BLOCK,
                FixedHeight::new(0),
                Priority::High,
            )
            .await?;
        Ok(fixed_block.inner)
    }

    async fn connected(&self) -> bool {
        self.network
            .diagnostic
            .session_by_chain(&self.remote_chain_addr)
            .is_some()
    }

    async fn broadcast<M: MessageCodec>(&self, endpoint: &str, msg: M) -> ProtocolResult<()> {
        let ctx = Context::new().with_value::<usize>("session_id", 1);
        self.network
            .users_cast::<M>(
                ctx,
                endpoint,
                vec![self.remote_chain_addr.clone()],
                msg,
                Priority::High,
            )
            .await
    }
}

fn full_node_hex_pubkey() -> String {
    let config: Config =
        common_config_parser::parse(&consts::CHAIN_CONFIG_PATH).expect("parse chain config.toml");
    let full_node = config
        .network
        .bootstraps
        .expect("config.toml full node")
        .pop()
        .expect("full node should be bootstrap");

    full_node.pubkey.as_string_trim0x()
}

fn full_node_chain_addr(hex_pubkey: &str) -> Address {
    let pubkey = hex::decode(hex_pubkey).expect("decode hex full node pubkey");
    Address::from_pubkey_bytes(pubkey.into()).expect("full node chain address")
}
