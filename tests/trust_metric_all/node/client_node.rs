use super::diagnostic::{
    TrustNewIntervalReq, TrustTwinEventReq, TwinEvent, GOSSIP_TRUST_NEW_INTERVAL,
    GOSSIP_TRUST_TWIN_EVENT,
};
use super::{
    consts,
    sync::{Sync, SyncError, SyncEvent},
};

use common_crypto::{PrivateKey, PublicKey, Secp256k1PrivateKey, ToPublicKey};
use core_consensus::message::{
    FixedBlock, FixedHeight, BROADCAST_HEIGHT, RPC_RESP_SYNC_PULL_BLOCK, RPC_SYNC_PULL_BLOCK,
};
use core_network::{
    DiagnosticEvent, NetworkConfig, NetworkService, NetworkServiceHandle, PeerId, PeerIdExt,
    TrustReport,
};
use derive_more::Display;
use protocol::{
    async_trait,
    traits::{Context, Gossip, MessageCodec, MessageHandler, Priority, Rpc, TrustFeedback},
    types::{Address, Block, BlockHeader, Hash, Proof},
    Bytes,
};

use std::{
    collections::HashSet,
    convert::TryFrom,
    iter::FromIterator,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
    str::FromStr,
};

#[derive(Debug, Display)]
pub enum ClientNodeError {
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

struct ReceiveRemoteHeight(Sync);

#[async_trait]
impl MessageHandler for ReceiveRemoteHeight {
    type Message = u64;

    async fn process(&self, _: Context, msg: u64) -> TrustFeedback {
        self.0.emit(DiagnosticEvent::RemoteHeight { height: msg });
        TrustFeedback::Neutral
    }
}

pub struct ClientNode {
    pub network:        NetworkServiceHandle,
    pub remote_peer_id: PeerId,
    pub priv_key:       Secp256k1PrivateKey,
    pub sync:           Sync,
}

pub async fn connect(
    full_node_port: u16,
    full_seckey: String,
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

    network.set_chain_id(Hash::from_hex(consts::CHAIN_ID).expect("chain id"));

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
    // # Panic
    pub async fn wait_connected(&self) {
        self.sync.wait_connected().await
    }

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

    pub async fn broadcast<M: MessageCodec>(&self, endpoint: &str, msg: M) -> ClientResult<()> {
        use Priority::High;

        let sid = match self.connected_session(&self.remote_peer_id) {
            Some(sid) => sid,
            None => return Err(ClientNodeError::NotConnected),
        };

        let ctx = Context::new().with_value::<usize>("session_id", sid);
        let peers = vec![Bytes::from(self.remote_peer_id.clone().into_bytes())];

        match self.multicast(ctx, endpoint, peers, msg, High).await {
            Err(_) if !self.connected() => Err(ClientNodeError::NotConnected),
            Err(e) => {
                let err_msg = format!("broadcast to {} {}", endpoint, e);
                Err(ClientNodeError::Unexpected(err_msg))
            }
            Ok(_) => Ok(()),
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

    pub async fn trust_twin_event(&self, event: TwinEvent) -> ClientResult<()> {
        self.broadcast(GOSSIP_TRUST_TWIN_EVENT, TrustTwinEventReq(event))
            .await?;

        let mut targets: HashSet<TwinEvent> = if event == TwinEvent::Both {
            HashSet::from_iter(vec![TwinEvent::Good, TwinEvent::Bad])
        } else {
            HashSet::from_iter(vec![event])
        };

        while !targets.is_empty() {
            let _ = match self.until_trust_processed().await? {
                TrustFeedback::Bad(_) => targets.remove(&TwinEvent::Bad),
                TrustFeedback::Good => targets.remove(&TwinEvent::Good),
                TrustFeedback::Worse(_) => targets.remove(&TwinEvent::Worse),
                TrustFeedback::Neutral | TrustFeedback::Fatal(_) => {
                    // No Fatal action yet
                    println!("skip neutral or fatal feedback");
                    continue;
                }
            };
        }

        Ok(())
    }

    pub async fn until_trust_processed(&self) -> ClientResult<TrustFeedback> {
        loop {
            let event = self.sync.recv().await?;
            match event {
                SyncEvent::TrustMetric(feedback) => return Ok(feedback),
                SyncEvent::RemoteHeight(_) => continue,
                _ => return Err(ClientNodeError::Unexpected(event.to_string())),
            }
        }
    }

    pub async fn trust_new_interval(&self) -> ClientResult<TrustReport> {
        self.broadcast(GOSSIP_TRUST_NEW_INTERVAL, TrustNewIntervalReq(0))
            .await?;

        loop {
            let event = self.sync.recv().await?;
            match event {
                SyncEvent::TrustReport(report) => return Ok(report),
                SyncEvent::Connected => {
                    return Err(ClientNodeError::Unexpected("connected".to_owned()))
                }
                SyncEvent::TrustMetric(_) | SyncEvent::RemoteHeight(_) => {
                    println!("skip event {}", event);
                    continue;
                }
            }
        }
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

fn mock_block(height: u64) -> Block {
    let block_hash = Hash::digest(Bytes::from("22"));
    let nonce = Hash::digest(Bytes::from("33"));
    let addr_str = "muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705";

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
        proposer: Address::from_str(addr_str).unwrap(),
        proof,
        validator_version: 1,
        validators: Vec::new(),
    };

    Block {
        header,
        ordered_tx_hashes: Vec::new(),
    }
}
