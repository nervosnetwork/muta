use std::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll},
    time::Duration,
};

use futures::{
    channel::mpsc::{self, Receiver, UnboundedSender},
    Future, Stream,
};
use log::debug;
use tentacle::{
    builder::MetaBuilder,
    bytes::Bytes,
    context::{ProtocolContext, ProtocolContextMutRef},
    service::{ProtocolHandle, ProtocolMeta},
    traits::ServiceProtocol,
    ProtocolId,
};
use tentacle_ping::{Event as PingEvent, PingHandler};

use crate::event::{PeerManagerEvent, RetryKind};

pub const NAME: &str = "chain_ping";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

struct PingEventReporter {
    inner: UnboundedSender<PeerManagerEvent>,

    mgr_shutdown: AtomicBool,
}

#[derive(derive_more::Constructor)]
struct EventTranslator {
    rx:       Receiver<PingEvent>,
    reporter: PingEventReporter,
}

impl Future for EventTranslator {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.reporter.is_mgr_shutdown() {
            return Poll::Ready(());
        }

        loop {
            let event = match Stream::poll_next(Pin::new(&mut self.as_mut().rx), cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(event)) => event,
            };

            let mgr_event = match event {
                PingEvent::Ping(ref pid) | PingEvent::Pong(ref pid, _) => {
                    PeerManagerEvent::PeerAlive { pid: pid.clone() }
                }
                PingEvent::Timeout(ref pid) => {
                    let kind = RetryKind::TimedOut;

                    PeerManagerEvent::RetryPeerLater {
                        pid: pid.clone(),
                        kind,
                    }
                }
                PingEvent::UnexpectedError(ref pid) => {
                    let kind = RetryKind::Other("ping unexpected error, maybe unstable network");

                    PeerManagerEvent::RetryPeerLater {
                        pid: pid.clone(),
                        kind,
                    }
                }
            };

            if self.reporter.inner.unbounded_send(mgr_event).is_err() {
                self.reporter.mgr_shutdown();
                return Poll::Ready(());
            }
        }

        Poll::Pending
    }
}

pub struct Ping {
    handler: PingHandler,
}

impl Ping {
    pub fn new(
        interval: Duration,
        timeout: Duration,
        sender: UnboundedSender<PeerManagerEvent>,
    ) -> Self {
        let reporter = PingEventReporter::new(sender);
        let (tx, rx) = mpsc::channel(1000);
        let handler = PingHandler::new(interval, timeout, tx);
        let translator = EventTranslator::new(rx, reporter);

        tokio::spawn(translator);

        Ping { handler }
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || ProtocolHandle::Callback(Box::new(self)))
            .build()
    }
}

impl ServiceProtocol for Ping {
    fn init(&mut self, ctx: &mut ProtocolContext) {
        self.handler.init(ctx)
    }

    fn connected(&mut self, ctx: ProtocolContextMutRef, version: &str) {
        self.handler.connected(ctx, version)
    }

    fn disconnected(&mut self, ctx: ProtocolContextMutRef) {
        self.handler.disconnected(ctx)
    }

    fn received(&mut self, ctx: ProtocolContextMutRef, data: Bytes) {
        self.handler.received(ctx, data)
    }

    fn notify(&mut self, ctx: &mut ProtocolContext, token: u64) {
        self.handler.notify(ctx, token)
    }
}

impl PingEventReporter {
    fn new(inner: UnboundedSender<PeerManagerEvent>) -> Self {
        PingEventReporter {
            inner,

            mgr_shutdown: AtomicBool::new(false),
        }
    }

    fn is_mgr_shutdown(&self) -> bool {
        self.mgr_shutdown.load(Ordering::SeqCst)
    }

    fn mgr_shutdown(&self) {
        debug!("network: ping: peer manager shutdown");

        self.mgr_shutdown.store(true, Ordering::SeqCst);
    }
}
