use super::common;

use core_network::{NetworkConfig, NetworkService, NetworkServiceHandle};
use common_crypto::{PrivateKey, Secp256k1PrivateKey};
use protocol::{
    async_trait,
    traits::{Context, Priority, Rpc, Gossip},
    types::{Block, Address},
    ProtocolResult,
};
use core_consensus::message::{FixedHeight, FixedBlock, RPC_SYNC_PULL_BLOCK, RPC_RESP_SYNC_PULL_BLOCK};
use core_mempool::{END_GOSSIP_NEW_TXS, MsgNewTxs};

use std::net::{SocketAddr, IpAddr, Ipv4Addr};

#[async_trait]
pub trait TestNodeRPC {
    async fn genesis_block(&self) -> ProtocolResult<Block>;
    async fn disconnected(&self) -> bool;
}

pub struct FullNode {
    pub pubkey: String,
    pub chain_addr: String,
    pub addr:   String,
}

pub struct TestNode {
    network: NetworkServiceHandle,
    remote_chain_addr: Address,
    priv_key: Secp256k1PrivateKey,
}

pub async fn make(full_node: FullNode, listen_port: u16) -> TestNode {
    let config = NetworkConfig::new()
        .ping_interval(Some(99999)) // disable ping interval to remove trust feedback good fromm it
        .peer_trust_metric(Some(5), None).expect("peer trust")
        .bootstraps(vec![(full_node.pubkey, full_node.addr)])
        .expect("test node config");

    let priv_key = Secp256k1PrivateKey::generate(&mut rand::rngs::OsRng);
    let remote_chain_addr = Address::from_hex(&full_node.chain_addr).expect("remote chain address");

    let mut network = NetworkService::new(config);
    let handle = network.handle();
    network.listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), listen_port)).await.expect("test node listen");

    network.register_rpc_response::<FixedBlock>(RPC_RESP_SYNC_PULL_BLOCK).expect("register consensus rpc pull block");

    tokio::spawn(network);

    TestNode {
        network: handle,
        remote_chain_addr,
        priv_key,
    }
}

#[async_trait]
impl TestNodeRPC for TestNode {
    async fn genesis_block(&self) -> ProtocolResult<Block> {
        let ctx = Context::new().with_value::<usize>("session_id", 1);
        let fixed_block = self.network.call::<FixedHeight, FixedBlock>(ctx, RPC_SYNC_PULL_BLOCK, FixedHeight::new(0), Priority::High).await?;
        Ok(fixed_block.inner)
    }

    async fn disconnected(&self) -> bool {
        let ctx = Context::new().with_value::<usize>("session_id", 1);
        let stx = common::gen_signed_tx(&self.priv_key, 199, true);
        let msg_stxs = MsgNewTxs {
            batch_stxs: vec![stx]
        };

        match self.network.users_cast::<MsgNewTxs>(ctx, END_GOSSIP_NEW_TXS, vec![self.remote_chain_addr.clone()], msg_stxs, Priority::High).await {
            Ok(_) => false,
            Err(e) => if e.to_string().contains("unconnected None") {
                false
            } else {
                true
            }
        }
    }
}
