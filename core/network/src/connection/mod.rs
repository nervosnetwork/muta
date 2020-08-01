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
    time::Duration,
};

use futures::{
    channel::mpsc::UnboundedReceiver, channel::mpsc::UnboundedSender, pin_mut, stream::Stream,
};
use log::debug;
use tentacle::{
    builder::ServiceBuilder, error::SendErrorKind, multiaddr::Multiaddr, secio::SecioKeyPair,
    service::Service,
};

use crate::{
    error::NetworkError,
    event::{ConnectionEvent, PeerManagerEvent},
    traits::{NetworkProtocol, SharedSessionBook},
};

pub struct ConnectionConfig {
    /// Secio keypair for stream encryption and peer identity
    pub secio_keypair: SecioKeyPair,

    /// Max stream window size
    pub max_frame_length: Option<usize>,

    /// Send buffer size
    pub send_buffer_size: Option<usize>,

    /// Write buffer size
    pub recv_buffer_size: Option<usize>,

    /// Max wait streams
    pub max_wait_streams: Option<usize>,

    /// Write timeout
    pub write_timeout: Option<u64>,
}

pub struct ConnectionService<P: NetworkProtocol> {
    inner: Service<ConnectionServiceKeeper>,

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

        let mut yamux_config = tentacle::yamux::Config::default();

        if let Some(max) = config.max_wait_streams {
            yamux_config.accept_backlog = max;
        }

        if let Some(timeout) = config.write_timeout {
            yamux_config.connection_write_timeout = Duration::from_secs(timeout);
        }

        builder = builder.yamux_config(yamux_config);

        if let Some(max) = config.max_frame_length {
            builder = builder.max_frame_length(max);
        }

        if let Some(size) = config.send_buffer_size {
            builder = builder.set_send_buffer_size(size);
        }

        if let Some(size) = config.recv_buffer_size {
            builder = builder.set_recv_buffer_size(size);
        }

        for proto_meta in protocol.metas().into_iter() {
            debug!("network: connection: insert protocol {}", proto_meta.name());
            builder = builder.insert_protocol(proto_meta);
        }

        ConnectionService {
            inner: builder.build(keeper),

            event_rx,
            pending_events: Default::default(),

            pin_protocol: PhantomData,
        }
    }

    pub async fn listen(&mut self, address: Multiaddr) -> Result<(), NetworkError> {
        self.inner.listen(address).await?;

        Ok(())
    }

    pub fn control<B: SharedSessionBook>(
        &self,
        mgr_tx: UnboundedSender<PeerManagerEvent>,
        book: B,
    ) -> ConnectionServiceControl<P, B> {
        let control_ref = self.inner.control();

        ConnectionServiceControl::new(control_ref.clone(), mgr_tx, book)
    }

    // BrokenPipe means service is closed.
    // WouldBlock means service is temporary unavailable.
    //
    // If WouldBlock is returned, we should try again later.
    pub fn process_event(&mut self, event: ConnectionEvent) {
        enum State {
            Closed,
            Busy, // limit to 2048 in tentacle
        }

        macro_rules! try_do {
            ($ctrl_op:expr) => {{
                let ret = $ctrl_op.map_err(|err| match &err {
                    SendErrorKind::BrokenPipe => State::Closed,
                    SendErrorKind::WouldBlock => State::Busy,
                });

                match ret {
                    Ok(_) => Ok(()),
                    Err(state) => match state {
                        State::Closed => return, // Early abort func
                        State::Busy => Err::<(), ()>(()),
                    },
                }
            }};
        }

        let control = self.inner.control();

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

            crate::service_ready!("connection service", inner.poll_next(ctx));
        }

        Poll::Pending
    }
}
