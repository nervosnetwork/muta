use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use futures::channel::mpsc::UnboundedSender;
use generic_channel::{Sender, TrySendError};
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
    reporter: UnboundedSender<PeerManagerEvent>,

    mgr_shutdown: AtomicBool,
}

pub struct Ping {
    handler: PingHandler<PingEventReporter>,
}

impl Ping {
    pub fn new(
        interval: Duration,
        timeout: Duration,
        sender: UnboundedSender<PeerManagerEvent>,
    ) -> Self {
        let reporter = PingEventReporter::new(sender);
        let handler = PingHandler::new(interval, timeout, reporter);

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
    fn new(reporter: UnboundedSender<PeerManagerEvent>) -> Self {
        PingEventReporter {
            reporter,

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

impl Sender<PingEvent> for PingEventReporter {
    fn try_send(&mut self, event: PingEvent) -> Result<(), TrySendError<PingEvent>> {
        if self.is_mgr_shutdown() {
            return Err(TrySendError::Disconnected(event));
        }

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

        self.reporter.unbounded_send(mgr_event).map_err(|err| {
            if err.is_full() {
                TrySendError::Full(event)
            } else {
                self.mgr_shutdown();

                TrySendError::Disconnected(event)
            }
        })
    }
}
