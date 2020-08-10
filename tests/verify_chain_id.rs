/// NOTE: Test may panic after drop full node future, which is
/// expected.
pub mod common;

use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;

use common_crypto::{PrivateKey, PublicKey, Secp256k1PrivateKey, ToPublicKey};
use core_consensus::message::{
    FixedBlock, FixedHeight, BROADCAST_HEIGHT, RPC_RESP_SYNC_PULL_BLOCK, RPC_SYNC_PULL_BLOCK,
};
use core_network::{
    DiagnosticEvent, NetworkConfig, NetworkService, NetworkServiceHandle, PeerId, PeerIdExt,
};
use derive_more::Display;
use protocol::traits::{Context, MessageCodec, MessageHandler, Priority, Rpc, TrustFeedback};
use protocol::types::{Block, Hash};
use protocol::{async_trait, Bytes};

use crate::common::available_port_pair;
use crate::common::node::consts;
use crate::common::node::full_node;
use crate::common::node::sync::{Sync, SyncError};

#[test]
fn should_be_disconnected_due_to_different_chain_id() {
    let (full_port, client_port) = available_port_pair();
    let mut rt = tokio::runtime::Runtime::new().expect("create runtime");
    let local = tokio::task::LocalSet::new();

    local.block_on(&mut rt, async move {
        let sync = Sync::new();
        let full_seckey = {
            let key = Secp256k1PrivateKey::generate(&mut rand::rngs::OsRng);
            hex::encode(key.to_bytes()).to_string()
        };
        tokio::task::spawn_local(full_node::run(full_port, full_seckey.clone(), sync.clone()));

        // Wait full node network initialization
        sync.wait().await;

        let chain_id = Hash::digest(Bytes::from_static(b"beautiful world"));
        let full_node_peer_id = full_node_peer_id(&full_seckey);
        let full_node_addr = format!("127.0.0.1:{}", full_port);

        let config = NetworkConfig::new()
            .ping_interval(consts::NETWORK_PING_INTERVAL)
            .peer_trust_metric(consts::NETWORK_TRUST_METRIC_INTERVAL, None)
            .expect("peer trust")
            .bootstraps(vec![(full_node_peer_id.to_base58(), full_node_addr)])
            .expect("test node config");

        let mut network = NetworkService::new(config);

        network.set_chain_id(chain_id);

        network
            .register_endpoint_handler(
                BROADCAST_HEIGHT,
                Box::new(ReceiveRemoteHeight(sync.clone())),
            )
            .expect("register remote height");

        let hook_fn = |sync: Sync| -> _ {
            Box::new(move |event: DiagnosticEvent| {
                // We only care connected event on client node
                if let DiagnosticEvent::NewSession = event {
                    sync.emit(event)
                }
            })
        };
        network.register_diagnostic_hook(hook_fn(sync.clone()));

        network
            .listen(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
                client_port,
            ))
            .await
            .expect("test node listen");
        tokio::spawn(network);

        match sync.recv().await {
            Err(SyncError::Disconected) => (),
            Err(err) => panic!("unexpected err {}", err),
            Ok(event) => panic!("unexpected event {}", event),
        }
    });
}

#[test]
fn should_be_connected_with_same_chain_id() {
    let (full_port, client_port) = available_port_pair();
    let mut rt = tokio::runtime::Runtime::new().expect("create runtime");
    let local = tokio::task::LocalSet::new();

    local.block_on(&mut rt, async move {
        let sync = Sync::new();
        let full_seckey = {
            let key = Secp256k1PrivateKey::generate(&mut rand::rngs::OsRng);
            hex::encode(key.to_bytes()).to_string()
        };
        tokio::task::spawn_local(full_node::run(full_port, full_seckey.clone(), sync.clone()));

        // Wait full node network initialization
        sync.wait().await;
        let chain_id = Hash::from_hex(consts::CHAIN_ID).expect("chain id");
        let client_node =
            connect(full_port, full_seckey, chain_id, client_port, sync.clone()).await;

        let block = client_node.get_block(0).await.expect("get genesis");
        assert_eq!(block.header.height, 0);
    });
}

#[derive(Debug, Display)]
enum ClientNodeError {
    #[display(fmt = "not connected")]
    NotConnected,

    #[display(fmt = "unexpected {}", _0)]
    Unexpected(String),
}
impl std::error::Error for ClientNodeError {}

impl From<SyncError> for ClientNodeError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::Recv(err) => ClientNodeError::Unexpected(err.to_string()),
            SyncError::Timeout => ClientNodeError::Unexpected(err.to_string()),
            SyncError::Disconected => ClientNodeError::NotConnected,
        }
    }
}

type ClientResult<T> = Result<T, ClientNodeError>;

struct ReceiveRemoteHeight(Sync);

#[async_trait]
impl MessageHandler for ReceiveRemoteHeight {
    type Message = u64;

    async fn process(&self, _: Context, msg: u64) -> TrustFeedback {
        self.0.emit(DiagnosticEvent::RemoteHeight { height: msg });
        TrustFeedback::Neutral
    }
}
struct ClientNode {
    pub network:        NetworkServiceHandle,
    pub remote_peer_id: PeerId,
    pub priv_key:       Secp256k1PrivateKey,
    pub sync:           Sync,
}

async fn connect(
    full_node_port: u16,
    full_seckey: String,
    chain_id: Hash,
    listen_port: u16,
    sync: Sync,
) -> ClientNode {
    let full_node_peer_id = full_node_peer_id(&full_seckey);
    let full_node_addr = format!("127.0.0.1:{}", full_node_port);

    let config = NetworkConfig::new()
        .ping_interval(consts::NETWORK_PING_INTERVAL)
        .peer_trust_metric(consts::NETWORK_TRUST_METRIC_INTERVAL, None)
        .expect("peer trust")
        .bootstraps(vec![(full_node_peer_id.to_base58(), full_node_addr)])
        .expect("test node config");
    let priv_key = Secp256k1PrivateKey::generate(&mut rand::rngs::OsRng);

    let mut network = NetworkService::new(config);
    let handle = network.handle();

    network.set_chain_id(chain_id);

    network
        .register_rpc_response::<FixedBlock>(RPC_RESP_SYNC_PULL_BLOCK)
        .expect("register consensus rpc response pull block");

    network
        .register_endpoint_handler(
            BROADCAST_HEIGHT,
            Box::new(ReceiveRemoteHeight(sync.clone())),
        )
        .expect("register remote height");

    let hook_fn = |sync: Sync| -> _ {
        Box::new(move |event: DiagnosticEvent| {
            // We only care connected event on client node
            if let DiagnosticEvent::NewSession = event {
                sync.emit(event)
            }
        })
    };
    network.register_diagnostic_hook(hook_fn(sync.clone()));

    network
        .listen(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            listen_port,
        ))
        .await
        .expect("test node listen");

    tokio::spawn(network);
    sync.wait_connected().await;

    ClientNode {
        network: handle,
        remote_peer_id: full_node_peer_id,
        priv_key,
        sync,
    }
}

impl ClientNode {
    pub fn connected(&self) -> bool {
        let diagnostic = &self.network.diagnostic;
        let opt_session = diagnostic.session(&self.remote_peer_id);

        self.sync.is_connected() && opt_session.is_some()
    }

    pub fn connected_session(&self, peer_id: &PeerId) -> Option<usize> {
        if !self.connected() {
            None
        } else {
            let diagnostic = &self.network.diagnostic;
            let opt_session = diagnostic.session(peer_id);

            opt_session.map(|sid| sid.value())
        }
    }

    pub async fn rpc<M, R>(&self, endpoint: &str, msg: M) -> ClientResult<R>
    where
        M: MessageCodec,
        R: MessageCodec,
    {
        let sid = match self.connected_session(&self.remote_peer_id) {
            Some(sid) => sid,
            None => return Err(ClientNodeError::NotConnected),
        };

        let ctx = Context::new().with_value::<usize>("session_id", sid);
        match self.call::<M, R>(ctx, endpoint, msg, Priority::High).await {
            Ok(resp) => Ok(resp),
            Err(e) if e.to_string().to_lowercase().contains("timeout") && !self.connected() => {
                Err(ClientNodeError::NotConnected)
            }
            Err(e) => {
                let err_msg = format!("rpc to {} {}", endpoint, e);
                Err(ClientNodeError::Unexpected(err_msg))
            }
        }
    }

    pub async fn get_block(&self, height: u64) -> ClientResult<Block> {
        let resp = self
            .rpc::<_, FixedBlock>(RPC_SYNC_PULL_BLOCK, FixedHeight::new(height))
            .await?;
        Ok(resp.inner)
    }
}

impl Deref for ClientNode {
    type Target = NetworkServiceHandle;

    fn deref(&self) -> &Self::Target {
        &self.network
    }
}

fn full_node_peer_id(full_seckey: &str) -> PeerId {
    let seckey = {
        let key = hex::decode(full_seckey).expect("hex private key string");
        Secp256k1PrivateKey::try_from(key.as_ref()).expect("valid private key")
    };
    let pubkey = seckey.pub_key();
    PeerId::from_pubkey_bytes(pubkey.to_bytes()).expect("valid public key")
}
