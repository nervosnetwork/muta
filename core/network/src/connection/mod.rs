mod control;
mod keeper;
pub use control::ConnectionServiceControl;
pub use keeper::ConnectionServiceKeeper;

use std::{
    collections::VecDeque,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    channel::mpsc::UnboundedReceiver,
    channel::mpsc::UnboundedSender,
    compat::{Compat01As03, Stream01CompatExt},
    pin_mut,
    stream::Stream,
};
use log::{debug, error};
use protocol::traits::Priority;
use tentacle::{
    builder::ServiceBuilder, error::Error as TentacleError, multiaddr::Multiaddr,
    secio::SecioKeyPair, service::Service,
};

use crate::{
    error::NetworkError,
    event::{ConnectionEvent, PeerManagerEvent},
    traits::NetworkProtocol,
};

pub struct ConnectionConfig {
    /// Secio keypair for stream encryption and peer identity
    pub secio_keypair: SecioKeyPair,

    /// Max stream window size
    pub max_frame_length: Option<usize>,
}

pub struct ConnectionService<P: NetworkProtocol> {
    // TODO: Remove Compat01As03 after tentacle supports std Future
    inner: Compat01As03<Service<ConnectionServiceKeeper>>,

    event_rx:       UnboundedReceiver<ConnectionEvent>,
    // Temporary store events for later processing under high load
    pending_events: VecDeque<ConnectionEvent>,

    // Indicate which protocol this connection service tries to open
    pin_protocol: PhantomData<P>,
}

impl<P: NetworkProtocol> ConnectionService<P> {
    pub fn new(
        protocol: P,
        config: ConnectionConfig,
        keeper: ConnectionServiceKeeper,
        event_rx: UnboundedReceiver<ConnectionEvent>,
    ) -> Self {
        let mut builder = ServiceBuilder::default()
            .key_pair(config.secio_keypair)
            .forever(true);

        if let Some(max) = config.max_frame_length {
            builder = builder.max_frame_length(max);
        }

        for proto_meta in protocol.metas().into_iter() {
            debug!("network: connection: insert protocol {}", proto_meta.name());
            builder = builder.insert_protocol(proto_meta);
        }

        ConnectionService {
            inner: builder.build(keeper).compat(),

            event_rx,
            pending_events: Default::default(),

            pin_protocol: PhantomData,
        }
    }

    pub fn listen(&mut self, address: Multiaddr) -> Result<(), NetworkError> {
        self.inner.get_mut().listen(address)?;

        Ok(())
    }

    pub fn control(
        &self,
        mgr_tx: UnboundedSender<PeerManagerEvent>,
    ) -> ConnectionServiceControl<P> {
        let control_ref = self.inner.get_ref().control();

        ConnectionServiceControl::new(control_ref.clone(), mgr_tx)
    }

    // NOTE: control.dial() and control.disconnect() both return same two
    // kinds of error: io::ErrorKind::BrokenPipe and io::ErrorKind::WouldBlock.
    //
    // BrokenPipe means service is closed.
    // WouldBlock means service is temporary unavailable.
    //
    // If WouldBlock is returned, we should try again later.
    pub fn process_event(&mut self, event: ConnectionEvent) {
        use std::io;

        enum State {
            Closed,
            Busy,                      // limit to 2048 in tentacle
            Unexpected(TentacleError), // Logic update required
        }

        macro_rules! try_do {
            ($ctrl_op:expr) => {{
                let ret = $ctrl_op.map_err(|err| match &err {
                    TentacleError::IoError(io_err) => match io_err.kind() {
                        io::ErrorKind::BrokenPipe => State::Closed,
                        io::ErrorKind::WouldBlock => State::Busy,
                        _ => State::Unexpected(err),
                    },
                    _ => State::Unexpected(err),
                });

                match ret {
                    Ok(_) => Ok(()),
                    Err(state) => match state {
                        State::Closed => return, // Early abort func
                        State::Busy => Err::<(), ()>(()),
                        State::Unexpected(e) => {
                            error!("network: connection: process_event() unexpected: {}", e);
                            Err::<(), ()>(())
                        }
                    },
                }
            }};
        }

        let control = self.inner.get_ref().control();

        match event {
            ConnectionEvent::Connect { addrs, .. } => {
                let mut pending_addrs = Vec::new();
                let target_protocol = P::target();

                for addr in addrs.into_iter() {
                    if let Err(()) = try_do!(control.dial(addr.clone(), target_protocol.clone())) {
                        pending_addrs.push(addr);
                    }
                }

                if !pending_addrs.is_empty() {
                    let pending_connect = ConnectionEvent::Connect {
                        addrs: pending_addrs,
                        proto: target_protocol,
                    };

                    self.pending_events.push_back(pending_connect);
                }
            }

            ConnectionEvent::Disconnect(sid) => {
                if let Err(()) = try_do!(control.disconnect(sid)) {
                    let pending_disconnect = ConnectionEvent::Disconnect(sid);

                    self.pending_events.push_back(pending_disconnect);
                }
            }

            ConnectionEvent::SendMsg { tar, msg, pri } => {
                let proto_id = P::message_proto_id();
                let tar2 = tar.clone();
                let msg2 = tentacle::bytes::Bytes::from(msg.as_ref());

                if let Err(()) = match pri {
                    Priority::High => try_do!(control.quick_filter_broadcast(tar2, proto_id, msg2)),
                    Priority::Normal => try_do!(control.filter_broadcast(tar2, proto_id, msg2)),
                } {
                    let pending_send_msg = ConnectionEvent::SendMsg { tar, msg, pri };

                    self.pending_events.push_back(pending_send_msg);
                }
            }
        }
    }
}

impl<P: NetworkProtocol + Unpin> Future for ConnectionService<P> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let serv_mut = &mut self.as_mut();

        // Process commands

        // Pending commands first
        let mut pending_events = std::mem::replace(&mut serv_mut.pending_events, VecDeque::new());
        for event in pending_events.drain(..) {
            debug!("network: pending event {}", event);

            serv_mut.process_event(event);
        }

        // Now received events
        // No-empty means service is temporary unavailable, try later
        while serv_mut.pending_events.is_empty() {
            let event_rx = &mut serv_mut.event_rx;
            pin_mut!(event_rx);

            let event = crate::service_ready!("connection service", event_rx.poll_next(ctx));
            debug!("network: event [{}]", event);

            serv_mut.process_event(event);
        }

        // Advance service state
        loop {
            let inner = &mut serv_mut.inner;
            pin_mut!(inner);

            let _ = crate::service_ready!("connection service", inner.poll_next(ctx));
        }

        Poll::Pending
    }
}
