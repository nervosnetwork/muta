use std::io;
use std::sync::{atomic::Ordering, Arc};
use std::{marker::Unpin, pin::Pin};

use futures::compat::{Compat01As03, Stream01CompatExt};
use futures::prelude::Stream;
use futures::task::{Context as FutTaskContext, Poll};
use log::{debug, error, warn};
use tentacle::{builder::ServiceBuilder, multiaddr::Multiaddr, traits::ServiceHandle};
// Cthulhu is a nightmare, let's contain it.
use tentacle::context::{ServiceContext, SessionContext};
use tentacle::error::Error as TentacleError;
use tentacle::service::{Service as Contained, ServiceControl};
use tentacle::service::{ServiceError, ServiceEvent};
use tentacle::ProtocolId;

use common_channel::Sender;

use crate::p2p::protocol::discovery::AddressManager;
use crate::p2p::protocol::{DiscoveryProtocol, TransmissionProtocol};
use crate::p2p::SessionMessage;
use crate::peer_manager::PeerManager;
use crate::{ConnectionPoolConfig, Context, Error};

const INIT_PROTO_ID: usize = 1;

pub use tentacle::bytes::Bytes;
pub use tentacle::service::{DialProtocol, TargetSession as Scope};
pub use tentacle::SessionId;

#[derive(Clone)]
pub struct Dialer {
    inner: ServiceControl,
}

impl Dialer {
    pub fn dial(&self, addr: Multiaddr, proto: DialProtocol) -> Result<(), Error> {
        self.inner.dial(addr, proto).map_err(Error::ConnectionError)
    }
}

#[derive(Clone)]
pub struct Outbound {
    proto_id: ProtocolId,
    inner:    ServiceControl,
}

impl Outbound {
    pub fn filter_broadcast(&self, scope: Scope, data: Bytes) -> Result<(), Error> {
        self.inner.filter_broadcast(scope, self.proto_id, data)?;

        Ok(())
    }

    pub fn quick_filter_broadcast(&self, scope: Scope, data: Bytes) -> Result<(), Error> {
        self.inner
            .quick_filter_broadcast(scope, self.proto_id, data)?;

        Ok(())
    }
}

pub struct ConnectionPoolService<M> {
    inner:              Compat01As03<Contained<ConnectionPoolHandle<M>>>,
    pub(crate) control: ServiceControl,
}

// TODO: reimplement discovery protocol so that we can remove 'static and lock
// using Pin.
impl<M> ConnectionPoolService<M>
where
    M: AddressManager + Send + 'static + PeerManager + Clone,
{
    pub fn init(
        ctx: Context,
        config: &ConnectionPoolConfig,
        inbound: Sender<SessionMessage>,
        mut peer_mgr: M,
    ) -> Result<Self, Error> {
        let trans =
            TransmissionProtocol::meta(ctx.clone(), ProtocolId::new(INIT_PROTO_ID), inbound);
        let disc = DiscoveryProtocol::meta(ProtocolId::new(INIT_PROTO_ID + 1), peer_mgr.clone());
        let handle = ConnectionPoolHandle::new(ctx, peer_mgr.clone());

        let mut inner = ServiceBuilder::default()
            .insert_protocol(trans)
            .insert_protocol(disc)
            .key_pair(config.key_pair.clone())
            .forever(true);

        if let Some(size) = config.send_buffer_size {
            inner = inner.set_send_buffer_size(size);
        }
        if let Some(size) = config.recv_buffer_size {
            inner = inner.set_recv_buffer_size(size);
        }
        let mut inner = inner.build(handle);

        let control = inner.control().clone();

        for addr in config.bootstrap_addresses.iter() {
            match inner.dial(addr.to_owned(), DialProtocol::All) {
                Ok(_) => peer_mgr.set_connected(addr),
                Err(err) => warn!("net [p2p]: dail bootstrap [addr: {}, error: {}]", addr, err),
            }
        }
        if peer_mgr.connected_count() == 0 && !config.bootstrap_addresses.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "fail to bootstrap",
            ))?;
        }

        inner.listen(config.listening_address.to_owned())?;
        // register ourself so that we don't connecet to ourself
        peer_mgr.add_addrs(vec![config.listening_address.to_owned()]);
        peer_mgr.set_connected(&config.listening_address);

        // TODO: insert shutdown signal, AtomicBool?
        let inner = inner.compat();
        Ok(ConnectionPoolService { inner, control })
    }

    pub fn dial(&self, addr: Multiaddr, target: DialProtocol) -> Result<(), Error> {
        self.control
            .dial(addr, target)
            .map_err(Error::ConnectionError)
    }

    // TODO: return impl trait, restrict api
    pub fn outbound(&self) -> Outbound {
        Outbound {
            proto_id: ProtocolId::new(INIT_PROTO_ID),
            inner:    self.control.clone(),
        }
    }

    pub fn dialer(&self) -> Dialer {
        Dialer {
            inner: self.control.clone(),
        }
    }
}

impl<M> Unpin for ConnectionPoolService<M> {}

impl<M> Stream for ConnectionPoolService<M>
where
    M: AddressManager + Send + 'static + PeerManager + Clone,
{
    type Item = Result<(), ()>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        ctx: &mut FutTaskContext<'_>,
    ) -> Poll<Option<Self::Item>> {
        Stream::poll_next(Pin::new(&mut self.inner), ctx)
    }
}

enum Status {
    Open,
    Close,
}

pub struct ConnectionPoolHandle<M> {
    ctx:      Context,
    peer_mgr: M,
}

impl<M> ConnectionPoolHandle<M>
where
    M: PeerManager,
{
    fn new(ctx: Context, peer_mgr: M) -> Self {
        ConnectionPoolHandle { ctx, peer_mgr }
    }

    fn update_status(&mut self, addr: &Multiaddr, status: Status) {
        match status {
            Status::Open => self.peer_mgr.set_connected(addr),
            Status::Close => self.peer_mgr.set_disconnected(addr),
        }
    }

    fn disconnect_session(&mut self, ctx: &mut ServiceContext, session_ctx: &Arc<SessionContext>) {
        let SessionContext {
            id,
            address,
            closed,
            ..
        } = session_ctx.as_ref();

        if closed.load(Ordering::Relaxed) {
            return;
        }

        debug!("net [p2p]: disconnect {}", address);

        self.peer_mgr.set_disconnected(address);
        if let Err(err) = ctx.disconnect(*id) {
            // Unable to disconnect? remove it from peer manager
            self.peer_mgr.remove_addrs(vec![address]);
            error!("net [p2p]: disconnect [addr: {}, err: {}]", address, err);
        }
    }
}

impl<M> ServiceHandle for ConnectionPoolHandle<M>
where
    M: PeerManager,
{
    fn handle_error(&mut self, ctx: &mut ServiceContext, err: ServiceError) {
        match err {
            ServiceError::DialerError { error, address }
            | ServiceError::ListenError { error, address } => match error {
                TentacleError::ConnectSelf | TentacleError::RepeatedConnection(_) => {
                    self.peer_mgr.add_addrs(vec![address.clone()]);
                    self.peer_mgr.set_connected(&address);
                }
                _ => error!("net [p2p]: dialer or listen error: {}", error),
            },
            ServiceError::ProtocolSelectError {
                session_context,
                proto_name,
            } => {
                warn!(
                    "net [p2p]: protocol select error: [name: {:?}, session: {:?}]. `None` name means unsupported protocol.",
                    proto_name, session_context
                );

                // If Timeout or other net problem, disconnect it and try
                // again later.
                self.disconnect_session(ctx, &session_context);

                // Unsupported protocol
                if proto_name.is_some() {
                    let addr = &session_context.address;
                    self.peer_mgr.remove_addrs(vec![addr]);
                }
            }
            ServiceError::ProtocolError {
                proto_id, error, ..
            } => {
                // Codec error
                // TODO: reboot p2p? but this codec error came from remote peer,
                // ddos attack?
                error!("net [p2p]: protocol: [id: {}, error: {}]", proto_id, error);
            }
            ServiceError::SessionTimeout { session_context } => {
                warn!("net [p2p]: session timeout: {:?}", session_context);

                self.disconnect_session(ctx, &session_context);
            }
            ServiceError::MuxerError {
                session_context,
                error,
            } => {
                warn!(
                    "net [p2p]: muxer: [session: {:?}, error: {}]",
                    session_context, error
                );
                self.disconnect_session(ctx, &session_context);
            }
            ServiceError::ProtocolHandleError { error, proto_id } => {
                // Will cause memory leaks/abnormal CPU usage
                error!(
                    "net [p2p]: protocol handle: [id: {}, error: {}]",
                    proto_id, error
                );

                // Report error back to network, reboot p2p service
                // FIXME: implement reboot
                if let Err(err) = self.ctx.err_tx.try_send(error.into()) {
                    error!("net [p2p]: fail to report error to network: {}", err);
                }
            }
            ServiceError::SessionBlocked { session_context } => {
                warn!("net [p2p]: session blocked: {:?}", session_context);
            }
        }
    }

    fn handle_event(&mut self, _: &mut ServiceContext, event: ServiceEvent) {
        debug!("net [p2p]: event: {:?}", event);

        match event {
            // Handle reconnection in p2p service stream
            ServiceEvent::SessionClose { session_context } => {
                self.update_status(&session_context.address, Status::Open)
            }
            ServiceEvent::SessionOpen { session_context } => {
                self.update_status(&session_context.address, Status::Close)
            }
            ServiceEvent::ListenClose { address } => self.update_status(&address, Status::Close),
            ServiceEvent::ListenStarted { address } => self.update_status(&address, Status::Open),
        }
    }
}
