use super::diagnostic::{
    TrustNewIntervalReq, TrustNewIntervalResp, TrustReport, TrustReportReq, TrustTwinEventReq,
    TrustTwinEventResp, TwinEvent, RPC_RESP_TRUST_NEW_INTERVAL, RPC_RESP_TRUST_REPORT,
    RPC_RESP_TRUST_TWIN_EVENT, RPC_TRUST_NEW_INTERVAL, RPC_TRUST_REPORT, RPC_TRUST_TWIN_EVENT,
};
use super::{config::Config, consts};

use common_crypto::{PrivateKey, Secp256k1PrivateKey};
use core_consensus::message::{
    FixedBlock, FixedHeight, RPC_RESP_SYNC_PULL_BLOCK, RPC_SYNC_PULL_BLOCK,
};
use core_network::{NetworkConfig, NetworkService, NetworkServiceHandle};
use derive_more::Display;
use protocol::{
    async_trait,
    traits::{Context, Gossip, MessageCodec, MessageHandler, Priority, Rpc, TrustFeedback},
    types::{Address, Block, BlockHeader, Hash, Proof},
    Bytes,
};

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
};

#[derive(Debug, Display)]
pub enum ClientNodeError {
    #[display(fmt = "not connected")]
    NotConnected,

    #[display(fmt = "unexpected {}", _0)]
    Unexpected(String),
}
impl std::error::Error for ClientNodeError {}

type ClientResult<T> = Result<T, ClientNodeError>;

struct DummyPullBlockRpcHandler(NetworkServiceHandle);

#[async_trait]
impl MessageHandler for DummyPullBlockRpcHandler {
    type Message = FixedHeight;

    async fn process(&self, ctx: Context, msg: FixedHeight) -> TrustFeedback {
        let block = FixedBlock::new(mock_block(msg.inner));
        self.0
            .response(ctx, RPC_RESP_SYNC_PULL_BLOCK, Ok(block), Priority::High)
            .await
            .expect("dummy response pull block");

        TrustFeedback::Neutral
    }
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
        .register_endpoint_handler(
            RPC_SYNC_PULL_BLOCK,
            Box::new(DummyPullBlockRpcHandler(handle.clone())),
        )
        .expect("register consensus rpc pull block");
    network
        .register_rpc_response::<FixedBlock>(RPC_RESP_SYNC_PULL_BLOCK)
        .expect("register consensus rpc response pull block");
    network
        .register_rpc_response::<TrustReport>(RPC_RESP_TRUST_REPORT)
        .expect("register trust report rpc response");
    network
        .register_rpc_response::<TrustNewIntervalResp>(RPC_RESP_TRUST_NEW_INTERVAL)
        .expect("register trigger trust new interval");
    network
        .register_rpc_response::<TrustTwinEventResp>(RPC_RESP_TRUST_TWIN_EVENT)
        .expect("register trigger basic trust test");

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

impl ClientNode {
    pub fn connected(&self) -> bool {
        self.network
            .diagnostic
            .session_by_chain(&self.remote_chain_addr)
            .is_some()
    }

    pub async fn broadcast<M: MessageCodec>(&self, endpoint: &str, msg: M) -> ClientResult<()> {
        let diagnostic = &self.network.diagnostic;
        let sid = match diagnostic.session_by_chain(&self.remote_chain_addr) {
            Some(sid) => sid,
            None => return Err(ClientNodeError::NotConnected),
        };

        let ctx = Context::new().with_value::<usize>("session_id", sid.value());
        let users = vec![self.remote_chain_addr.clone()];
        if let Err(e) = self
            .users_cast::<M>(ctx, endpoint, users, msg, Priority::High)
            .await
        {
            // Sleep a while to ensure our peer manager to process disconnect event
            tokio::time::delay_for(std::time::Duration::from_secs(2)).await;

            if !self.connected() {
                Err(ClientNodeError::NotConnected)
            } else {
                Err(ClientNodeError::Unexpected(format!(
                    "broadcast to {} {}",
                    endpoint, e
                )))
            }
        } else {
            Ok(())
        }
    }

    pub async fn rpc<M: MessageCodec, R: MessageCodec>(
        &self,
        endpoint: &str,
        msg: M,
    ) -> ClientResult<R> {
        let diagnostic = &self.network.diagnostic;
        let sid = match diagnostic.session_by_chain(&self.remote_chain_addr) {
            Some(sid) => sid,
            None => return Err(ClientNodeError::NotConnected),
        };

        let ctx = Context::new().with_value::<usize>("session_id", sid.value());
        match self.call::<M, R>(ctx, endpoint, msg, Priority::High).await {
            Ok(resp) => Ok(resp),
            Err(e)
                if e.to_string().contains("RpcTimeout")
                    || e.to_string().contains("rpc timeout") =>
            {
                // Sleep a while to ensure our peer manager to process disconnect event
                tokio::time::delay_for(std::time::Duration::from_secs(10)).await;

                if !self.connected() {
                    Err(ClientNodeError::NotConnected)
                } else {
                    Err(ClientNodeError::Unexpected(format!(
                        "rpc to {} {}",
                        endpoint, e
                    )))
                }
            }
            Err(e) => Err(ClientNodeError::Unexpected(format!(
                "rpc to {} {}",
                endpoint, e
            ))),
        }
    }

    pub async fn genesis_block(&self) -> ClientResult<Block> {
        let resp = self
            .rpc::<_, FixedBlock>(RPC_SYNC_PULL_BLOCK, FixedHeight::new(0))
            .await?;
        Ok(resp.inner)
    }

    pub async fn trust_report(&self) -> ClientResult<TrustReport> {
        self.rpc(RPC_TRUST_REPORT, TrustReportReq(0)).await
    }

    pub async fn trust_new_interval(&self) -> ClientResult<TrustReport> {
        self.rpc(RPC_TRUST_NEW_INTERVAL, TrustNewIntervalReq(0))
            .await?;

        self.until_new_interval_report().await
    }

    pub async fn trust_twin_event(&self, event: TwinEvent) -> ClientResult<()> {
        self.rpc::<_, TrustTwinEventResp>(RPC_TRUST_TWIN_EVENT, TrustTwinEventReq(event))
            .await?;
        Ok(())
    }

    pub async fn until_trust_report_changed(
        &self,
        last_report: &TrustReport,
    ) -> ClientResult<TrustReport> {
        let mut count = 30u8;
        while count > 0 {
            count -= 1;
            let report = self.trust_report().await?;
            if report.good_events != last_report.good_events
                || report.bad_events != last_report.bad_events
            {
                return Ok(report);
            }
            tokio::time::delay_for(std::time::Duration::from_millis(500)).await;
        }

        panic!("until trust report timeout");
    }

    async fn until_new_interval_report(&self) -> ClientResult<TrustReport> {
        let mut count = 30u8;
        while count > 0 {
            count -= 1;
            let report = self.trust_report().await?;
            if report.good_events == 0 && report.bad_events == 0 {
                return Ok(report);
            }
            tokio::time::delay_for(std::time::Duration::from_millis(500)).await;
        }

        panic!("until trust new interval timeout");
    }
}

impl Deref for ClientNode {
    type Target = NetworkServiceHandle;

    fn deref(&self) -> &Self::Target {
        &self.network
    }
}

fn full_node_hex_pubkey() -> String {
    let config: Config =
        common_config_parser::parse(&consts::CHAIN_CONFIG_PATH).expect("parse chain config.toml");

    let mut bootstraps = config.network.bootstraps.expect("config.toml full node");
    let full_node = bootstraps.pop().expect("there should be one bootstrap");

    full_node.pubkey.as_string_trim0x()
}

fn full_node_chain_addr(hex_pubkey: &str) -> Address {
    let pubkey = hex::decode(hex_pubkey).expect("decode hex full node pubkey");
    Address::from_pubkey_bytes(pubkey.into()).expect("full node chain address")
}

fn mock_block(height: u64) -> Block {
    let block_hash = Hash::digest(Bytes::from("22"));
    let nonce = Hash::digest(Bytes::from("33"));
    let addr_str = "0xCAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";

    let proof = Proof {
        height: 0,
        round: 0,
        block_hash,
        signature: Default::default(),
        bitmap: Default::default(),
    };

    let header = BlockHeader {
        chain_id: nonce.clone(),
        height,
        exec_height: height - 1,
        prev_hash: nonce.clone(),
        timestamp: 1000,
        order_root: nonce.clone(),
        order_signed_transactions_hash: nonce.clone(),
        confirm_root: Vec::new(),
        state_root: nonce,
        receipt_root: Vec::new(),
        cycles_used: vec![999_999],
        proposer: Address::from_hex(addr_str).unwrap(),
        proof,
        validator_version: 1,
        validators: Vec::new(),
    };

    Block {
        header,
        ordered_tx_hashes: Vec::new(),
    }
}
