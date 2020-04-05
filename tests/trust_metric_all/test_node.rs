use core_network::{NetworkConfig, NetworkService, NetworkServiceHandle};
use protocol::{
    async_trait,
    traits::{Context, Priority, Rpc},
    types::Block,
    ProtocolResult,
};
use core_consensus::message::{FixedHeight, FixedBlock, RPC_SYNC_PULL_BLOCK, RPC_RESP_SYNC_PULL_BLOCK};

use std::net::{SocketAddr, IpAddr, Ipv4Addr};

#[async_trait]
pub trait TestNodeRPC {
    async fn genesis_block(&self) -> ProtocolResult<Block>;
}

pub struct FullNode {
    pub pubkey: String,
    pub addr:   String,
}

pub async fn make(full_node: FullNode, listen_port: u16) -> NetworkServiceHandle {
    let config = NetworkConfig::new()
        .ping_interval(Some(99999)) // disable ping interval to remove trust feedback good fromm it
        .bootstraps(vec![(full_node.pubkey, full_node.addr)])
        .expect("test node config");

    let mut network = NetworkService::new(config);
    let handle = network.handle();
    network.listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), listen_port)).await.expect("test node listen");

    network.register_rpc_response::<FixedBlock>(RPC_RESP_SYNC_PULL_BLOCK).expect("register consensus rpc pull block");

    tokio::spawn(network);

    handle
}

#[async_trait]
impl TestNodeRPC for NetworkServiceHandle {
    async fn genesis_block(&self) -> ProtocolResult<Block> {
        let ctx = Context::new().with_value::<usize>("session_id", 1);
        let fixed_block = self.call::<FixedHeight, FixedBlock>(ctx, RPC_SYNC_PULL_BLOCK, FixedHeight::new(0), Priority::High).await?;
        Ok(fixed_block.inner)
    }
}
