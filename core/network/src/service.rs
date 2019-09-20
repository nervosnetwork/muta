use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context as TaskContext, Poll},
};

use async_trait::async_trait;
use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    future::TryFutureExt,
    pin_mut,
    stream::Stream,
    task::AtomicWaker,
};
use log::{debug, error, info};
use protocol::{
    traits::{Context, Gossip, MessageCodec, MessageHandler, Priority, Rpc},
    types::UserAddress,
    ProtocolResult,
};

use crate::{
    common::{socket_to_multi_addr, HeartBeat},
    compression::Snappy,
    connection::{
        ConnectionConfig, ConnectionService, ConnectionServiceControl, ConnectionServiceKeeper,
    },
    endpoint::{Endpoint, EndpointScheme},
    error::NetworkError,
    event::{ConnectionEvent, PeerManagerEvent},
    message::RawSessionMessage,
    outbound::{NetworkGossip, NetworkRpc},
    peer_manager::{DiscoveryAddrManager, PeerManager, PeerManagerConfig},
    protocols::CoreProtocol,
    reactor::{MessageRouter, Reactor},
    rpc_map::RpcMap,
    NetworkConfig,
};

#[derive(Clone)]
pub struct NetworkServiceHandle {
    gossip: NetworkGossip<ConnectionServiceControl<CoreProtocol>, Snappy>,
    rpc:    NetworkRpc<ConnectionServiceControl<CoreProtocol>, Snappy>,
}

#[async_trait]
impl Gossip for NetworkServiceHandle {
    async fn broadcast<M>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        self.gossip.broadcast(cx, end, msg, p).await
    }

    async fn users_cast<M>(
        &self,
        cx: Context,
        end: &str,
        users: Vec<UserAddress>,
        msg: M,
        p: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        self.gossip.users_cast(cx, end, users, msg, p).await
    }
}

#[async_trait]
impl Rpc for NetworkServiceHandle {
    async fn call<M, R>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<R>
    where
        M: MessageCodec,
        R: MessageCodec,
    {
        self.rpc.call(cx, end, msg, p).await
    }

    async fn response<M>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        self.rpc.response(cx, end, msg, p).await
    }
}

enum NetworkConnectionService {
    NoListen(ConnectionService<CoreProtocol>), // no listen address yet
    Ready(ConnectionService<CoreProtocol>),
}

pub struct NetworkService {
    sys_rx: UnboundedReceiver<NetworkError>,

    // Heart beats
    conn_tx:    UnboundedSender<ConnectionEvent>,
    mgr_tx:     UnboundedSender<PeerManagerEvent>,
    raw_msg_tx: UnboundedSender<RawSessionMessage>,
    heart_beat: Option<HeartBeat>,
    hb_waker:   Arc<AtomicWaker>,

    // Config backup
    config: NetworkConfig,

    // Public service components
    gossip:  NetworkGossip<ConnectionServiceControl<CoreProtocol>, Snappy>,
    rpc:     NetworkRpc<ConnectionServiceControl<CoreProtocol>, Snappy>,
    rpc_map: Arc<RpcMap>,

    // Core service
    net_conn_srv: Option<NetworkConnectionService>,
    peer_mgr:     Option<PeerManager>,
    router:       Option<MessageRouter<Snappy>>,
}

impl NetworkService {
    pub fn new(config: NetworkConfig) -> Self {
        let (mgr_tx, mgr_rx) = unbounded();
        let (conn_tx, conn_rx) = unbounded();
        let (raw_msg_tx, raw_msg_rx) = unbounded();
        let (sys_tx, sys_rx) = unbounded();

        let hb_waker = Arc::new(AtomicWaker::new());
        let heart_beat = HeartBeat::new(Arc::clone(&hb_waker), config.heart_beat_interval);

        let mgr_config = PeerManagerConfig::from(&config);
        let conn_config = ConnectionConfig::from(&config);

        // Build peer manager
        let mut peer_mgr = PeerManager::new(mgr_config, mgr_rx, conn_tx.clone());
        let peer_mgr_handle = peer_mgr.handle();

        if config.enable_persistence {
            peer_mgr.enable_persistence();
        }

        if let Err(err) = peer_mgr.load_peers() {
            error!("network: peer manager: load peers failure: {}", err);
        }

        if !config.bootstraps.is_empty() {
            peer_mgr.bootstrap();
        }

        // Build service protocol
        let disc_sync_interval = config.discovery_sync_interval;
        let disc_addr_mgr = DiscoveryAddrManager::new(peer_mgr_handle, mgr_tx.clone());
        let proto = CoreProtocol::build()
            .ping(config.ping_interval, config.ping_timeout, mgr_tx.clone())
            .discovery(disc_addr_mgr, disc_sync_interval)
            .transmitter(raw_msg_tx.clone())
            .build();

        // Build connection service
        let keeper = ConnectionServiceKeeper::new(mgr_tx.clone(), sys_tx.clone());
        let conn_srv = ConnectionService::<CoreProtocol>::new(proto, conn_config, keeper, conn_rx);
        let conn_ctrl = conn_srv.control(mgr_tx.clone());

        // Build public service components
        let rpc_map = Arc::new(RpcMap::new());
        let gossip = NetworkGossip::new(conn_ctrl.clone(), Snappy);
        let rpc_map_clone = Arc::clone(&rpc_map);
        let rpc = NetworkRpc::new(conn_ctrl.clone(), Snappy, rpc_map_clone);
        let router = MessageRouter::new(raw_msg_rx, Snappy, sys_tx.clone());

        NetworkService {
            sys_rx,

            conn_tx: conn_tx.clone(),
            mgr_tx: mgr_tx.clone(),
            raw_msg_tx: raw_msg_tx.clone(),
            hb_waker,
            heart_beat: Some(heart_beat),

            config,

            gossip,
            rpc,
            rpc_map,

            net_conn_srv: Some(NetworkConnectionService::NoListen(conn_srv)),
            peer_mgr: Some(peer_mgr),
            router: Some(router),
        }
    }

    pub fn register_endpoint_handler<M>(
        &mut self,
        end: &str,
        handler: Box<dyn MessageHandler<Message = M>>,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        let endpoint = end.parse::<Endpoint>()?;
        let (msg_tx, msg_rx) = unbounded();

        if endpoint.scheme() == EndpointScheme::RpcResponse {
            let err = "use register_rpc_response() instead".to_owned();

            return Err(NetworkError::UnexpectedScheme(err).into());
        }

        if let Some(router) = &mut self.router {
            router.register_reactor(endpoint, msg_tx);

            let reactor = Reactor::new(msg_rx, handler, Arc::clone(&self.rpc_map));
            runtime::spawn(reactor);
        }

        Ok(())
    }

    // Currently rpc response dont invoke message handler, so we create a dummy
    // for it.
    pub fn register_rpc_response<M>(&mut self, end: &str) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        let endpoint = end.parse::<Endpoint>()?;
        let (msg_tx, msg_rx) = unbounded();

        if endpoint.scheme() != EndpointScheme::RpcResponse {
            return Err(NetworkError::UnexpectedScheme(end.to_owned()).into());
        }

        if let Some(router) = &mut self.router {
            router.register_reactor(endpoint, msg_tx);

            let reactor = Reactor::<M>::rpc_resp(msg_rx, Arc::clone(&self.rpc_map));
            runtime::spawn(reactor);
        }

        Ok(())
    }

    pub fn handle(&self) -> NetworkServiceHandle {
        NetworkServiceHandle {
            gossip: self.gossip.clone(),
            rpc:    self.rpc.clone(),
        }
    }

    pub fn listen(&mut self, socket_addr: SocketAddr) -> ProtocolResult<()> {
        if let Some(NetworkConnectionService::NoListen(conn_srv)) = &mut self.net_conn_srv {
            debug!("network: connection: listen to {}", socket_addr);

            conn_srv.listen(socket_to_multi_addr(socket_addr))?;

            // Update service state
            if let Some(NetworkConnectionService::NoListen(conn_srv)) = self.net_conn_srv.take() {
                self.net_conn_srv = Some(NetworkConnectionService::Ready(conn_srv));
            } else {
                unreachable!("connection service must be there");
            }
        }

        Ok(())
    }
}

impl Future for NetworkService {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        debug!("network: service polled");

        self.hb_waker.register(ctx.waker());

        macro_rules! service_ready {
            ($poll:expr) => {
                match $poll {
                    Poll::Pending => break,
                    Poll::Ready(Some(v)) => v,
                    Poll::Ready(None) => {
                        info!("network shutdown");

                        return Poll::Ready(());
                    }
                }
            };
        }

        // Preflight
        if let Some(conn_srv) = self.net_conn_srv.take() {
            let conn_srv = match conn_srv {
                NetworkConnectionService::NoListen(mut conn_srv) => {
                    conn_srv
                        .listen(self.config.default_listen.clone())
                        .expect("fail to listen default address");

                    conn_srv
                }
                NetworkConnectionService::Ready(conn_srv) => conn_srv,
            };

            runtime::spawn(conn_srv);
        }

        if let Some(peer_mgr) = self.peer_mgr.take() {
            runtime::spawn(peer_mgr);
        }

        if let Some(router) = self.router.take() {
            runtime::spawn(router);
        }

        // Heart beats
        if let Some(heart_beat) = self.heart_beat.take() {
            let heart_beat = heart_beat.map_err(|_| {
                error!("network: fatal: asystole, no ecg now");
            });

            runtime::spawn(heart_beat);
        }

        // TODO: Reboot ceased service? Right now we just assume that it's
        // normal shutdown, simple log it and let it go.
        //
        // let it go ~~~ , let it go ~~~
        // i am one with the wind and sky
        // let it go, let it go
        // you'll never see me cry
        // bla bla bal ~~~
        if self.conn_tx.is_closed() {
            info!("network: connection service closed");
        }

        if self.mgr_tx.is_closed() {
            info!("network: peer manager closed");
        }

        if self.raw_msg_tx.is_closed() {
            info!("network: message router closed");
        }

        // Process system error report
        loop {
            let sys_rx = &mut self.as_mut().sys_rx;
            pin_mut!(sys_rx);

            let sys_err = service_ready!(sys_rx.poll_next(ctx));
            error!("network: system error: {}", sys_err);
        }

        Poll::Pending
    }
}
