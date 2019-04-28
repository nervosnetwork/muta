use bytes::Bytes;
use futures::prelude::{Async, Stream};
use futures::sync::mpsc::{channel, Receiver, Sender};
use tentacle::context::{ProtocolContext, ProtocolContextMutRef};
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::{builder::MetaBuilder, secio::PeerId, traits::ServiceProtocol, ProtocolId};
use tentacle_ping::{Event, PingHandler};

use std::time::Duration;

/// Protocol name (handshake)
pub const PROTOCOL_NAME: &str = "ping";

/// Protocol support versions
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

/// Internal event channel buffer size
pub const PING_EVENT_CHANNEL_BUFFER: usize = 24;

/// Internal ping interval (seconds)
pub const PING_INTERVAL: u64 = 5;

/// Internal ping timeout (seconds)
pub const PING_TIMEOUT: u64 = 15;

/// The enum for ping behavior
#[derive(Debug)]
pub enum Behavior {
    /// Peer send `ping` event
    Ping,
    /// Peer response `pong` event with relative elapsed time (ms)
    Pong(Duration),
    /// Peer response timeout
    Timeout,
    /// Unexpected error
    UnexpectedError,
}

/// Peer manager for ping protocol
pub trait PeerManager: Clone + Send {
    /// Update peer connective status to manager
    fn update_peer_status(&mut self, peer_id: &PeerId, kind: Behavior);
}

/// Protocol for ping
pub struct PingProtocol<TPeerManager> {
    ping_rx:  Receiver<Event>,
    peer_mgr: TPeerManager,

    inner: PingHandler<Sender<Event>>,
}

impl<TPeerManager> PingProtocol<TPeerManager>
where
    TPeerManager: PeerManager + Clone + 'static,
{
    /// Build a PingProtocol instance
    pub fn build(id: ProtocolId, peer_mgr: TPeerManager) -> ProtocolMeta {
        MetaBuilder::default()
            .id(id)
            .name(name!(PROTOCOL_NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || {
                let (ping_tx, ping_rx) = channel(PING_EVENT_CHANNEL_BUFFER);

                let interval = Duration::from_secs(PING_INTERVAL);
                let timeout = Duration::from_secs(PING_TIMEOUT);
                let inner = PingHandler::new(interval, timeout, ping_tx);

                let boxed_proto = Box::new(PingProtocol {
                    ping_rx,
                    peer_mgr: peer_mgr.clone(),

                    inner,
                });

                ProtocolHandle::Callback(boxed_proto)
            })
            .build()
    }

    pub(crate) fn do_init(&mut self, proto_ctx: &mut ProtocolContext) {
        self.inner.init(proto_ctx)
    }

    pub(crate) fn do_connec(&mut self, proto_ctx: ProtocolContextMutRef, version: &str) {
        self.inner.connected(proto_ctx, version)
    }

    pub(crate) fn do_disc(&mut self, proto_ctx: ProtocolContextMutRef) {
        self.inner.disconnected(proto_ctx)
    }

    pub(crate) fn do_recv(&mut self, proto_ctx: ProtocolContextMutRef, data: Bytes) {
        self.inner.received(proto_ctx, data)
    }

    pub(crate) fn do_notify(&mut self, proto_ctx: &mut ProtocolContext, token: u64) {
        self.inner.notify(proto_ctx, token)
    }

    pub(crate) fn do_peer_update(&mut self, proto_ctx: &mut ProtocolContext) {
        if let Ok(Async::Ready(Some(event))) = self.ping_rx.poll() {
            let (peer_id, behavior) = match event {
                Event::Ping(peer_id) => (peer_id, Behavior::Ping),
                Event::Pong(peer_id, elapsed) => (peer_id, Behavior::Pong(elapsed)),
                Event::Timeout(peer_id) => (peer_id, Behavior::Timeout),
                Event::UnexpectedError(peer_id) => (peer_id, Behavior::UnexpectedError),
            };

            self.peer_mgr.update_peer_status(&peer_id, behavior);
        }

        self.inner.poll(proto_ctx)
    }
}

impl<TPeerManager> ServiceProtocol for PingProtocol<TPeerManager>
where
    TPeerManager: PeerManager + 'static,
{
    fn init(&mut self, proto_ctx: &mut ProtocolContext) {
        self.do_init(proto_ctx)
    }

    fn connected(&mut self, proto_ctx: ProtocolContextMutRef, version: &str) {
        self.do_connec(proto_ctx, version)
    }

    fn disconnected(&mut self, proto_ctx: ProtocolContextMutRef) {
        self.do_disc(proto_ctx)
    }

    fn received(&mut self, proto_ctx: ProtocolContextMutRef, data: Bytes) {
        self.do_recv(proto_ctx, data)
    }

    fn notify(&mut self, proto_ctx: &mut ProtocolContext, token: u64) {
        self.do_notify(proto_ctx, token)
    }

    fn poll(&mut self, proto_ctx: &mut ProtocolContext) {
        self.do_peer_update(proto_ctx)
    }
}
