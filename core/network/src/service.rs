use std::sync::Arc;
use std::{marker::Unpin, pin::Pin};

use futures::prelude::{FutureExt, Stream, StreamExt, TryFutureExt};
use futures::task::{Context as FutTaskContext, Poll};
use log::error;

use common_channel::{bounded, Receiver};
use core_runtime::{Consensus, Storage, TransactionPool};

use crate::p2p::{conn_pool::ConnectionPoolService, Dialer, SessionMessage};
use crate::peer_manager::DefaultPeerManager;
use crate::{
    CallbackMap, Config, ConnectionPoolConfig, Context, Error, InboundHandle, OutboundHandle,
};

pub const PEER_MANAGER_ROUTINE_INTERVAL: u64 = 5;

pub struct PartialService {
    ctx:    Context,
    err_rx: Receiver<Error>,
    config: Config,

    peer_mgr: DefaultPeerManager,
    outbound: OutboundHandle,
    dialer:   Dialer,

    pool_config: ConnectionPoolConfig,
    conn_pool:   ConnectionPoolService<DefaultPeerManager>,
    msg_rx:      Receiver<SessionMessage>,

    callback: Arc<CallbackMap>,
}

impl PartialService {
    pub fn new(config: Config) -> Result<Self, Error> {
        let (err_tx, err_rx) = bounded(20);
        let ctx = Context::new(err_tx);

        let peer_mgr = DefaultPeerManager::new(config.max_connections);

        let (msg_tx, msg_rx) = bounded(20);
        let pool_config = ConnectionPoolConfig::from_config(&config)?;
        let conn_pool =
            ConnectionPoolService::init(ctx.clone(), &pool_config, msg_tx, peer_mgr.clone())?;

        let callback = Arc::new(CallbackMap::new());
        let outbound = OutboundHandle::new(Arc::clone(&callback), conn_pool.outbound());
        let dialer = conn_pool.dialer();

        Ok(PartialService {
            ctx,
            err_rx,
            config,

            peer_mgr,
            outbound,
            dialer,

            pool_config,
            conn_pool,
            msg_rx,

            callback,
        })
    }

    pub fn outbound(&self) -> OutboundHandle {
        self.outbound.clone()
    }

    pub fn build<T, C, S>(
        self,
        tx_pool: Arc<T>,
        consensus: Arc<C>,
        storage: Arc<S>,
    ) -> Service<T, C, S>
    where
        T: TransactionPool + 'static,
        C: Consensus + 'static,
        S: Storage + 'static,
    {
        let inbound = InboundHandle::new(
            Arc::clone(&self.callback),
            self.msg_rx,
            self.outbound.clone(),
            Arc::clone(&tx_pool),
            Arc::clone(&consensus),
            Arc::clone(&storage),
        );

        Service {
            ctx: self.ctx,
            err_rx: self.err_rx,
            config: self.config,

            peer_mgr: self.peer_mgr,
            inbound: Some(inbound),
            outbound: self.outbound,
            dialer: self.dialer,

            conn_pool: Some(self.conn_pool),
            pool_config: self.pool_config,

            tx_pool,
            consensus,
            storage,
        }
    }
}

// TODO: implement reboot, remove dead_code
#[allow(dead_code)]
pub struct Service<T, C, S>
where
    T: TransactionPool + 'static,
    C: Consensus + 'static,
    S: Storage + 'static,
{
    ctx:    Context,
    err_rx: Receiver<Error>,
    config: Config,

    peer_mgr: DefaultPeerManager,
    inbound:  Option<InboundHandle<T, C, S>>,
    outbound: OutboundHandle,
    dialer:   Dialer,

    conn_pool:   Option<ConnectionPoolService<DefaultPeerManager>>,
    pool_config: ConnectionPoolConfig,

    tx_pool:   Arc<T>,
    consensus: Arc<C>,
    storage:   Arc<S>,
}

impl<T, C, S> Service<T, C, S>
where
    T: TransactionPool + 'static,
    C: Consensus + 'static,
    S: Storage + 'static,
{
    pub async fn run(mut self) {
        // TODO: remove unwrap
        let conn_pool = self.conn_pool.take().unwrap();
        tokio::spawn(
            conn_pool
                .for_each(async move |_| ())
                .unit_error()
                .boxed()
                .compat(),
        );

        let inbound = self.inbound.take().unwrap();
        tokio::spawn(
            inbound
                .for_each(async move |_| ())
                .unit_error()
                .boxed()
                .compat(),
        );

        let peer_mgr = self.peer_mgr.clone();
        tokio::spawn(
            peer_mgr
                .run(self.dialer.clone(), PEER_MANAGER_ROUTINE_INTERVAL)
                .unit_error()
                .boxed()
                .compat(),
        );

        await!(self.for_each(async move |_| ()))
    }

    pub fn outbound(&self) -> OutboundHandle {
        self.outbound.clone()
    }
}

impl<T, C, S> Unpin for Service<T, C, S>
where
    T: TransactionPool + 'static,
    C: Consensus + 'static,
    S: Storage + 'static,
{
}

impl<T, C, S> Stream for Service<T, C, S>
where
    T: TransactionPool + 'static,
    C: Consensus + 'static,
    S: Storage + 'static,
{
    type Item = ();

    fn poll_next(
        mut self: Pin<&mut Self>,
        ctx: &mut FutTaskContext<'_>,
    ) -> Poll<Option<Self::Item>> {
        // Error reported
        match Stream::poll_next(Pin::new(&mut self.err_rx), ctx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(err)) => {
                // TODO: handle error, only fatal error should be reporthere
                // should reboot network service
                error!("net: fatal error: {:?}", err);
                Poll::Ready(Some(()))
            }
        }
    }
}
