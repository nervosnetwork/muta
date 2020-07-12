use super::protocol::PingEvent;
use crate::event::{MisbehaviorKind, PeerManagerEvent};

use futures::{
    channel::mpsc::{Receiver, UnboundedSender},
    Future, Stream,
};
use log::debug;

use std::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll},
};

pub struct PingEventReporter {
    inner:        UnboundedSender<PeerManagerEvent>,
    mgr_shutdown: AtomicBool,
}

impl PingEventReporter {
    pub fn new(inner: UnboundedSender<PeerManagerEvent>) -> Self {
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

#[derive(derive_more::Constructor)]
pub struct EventTranslator {
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
                PingEvent::Ping(ref _pid) => continue,
                PingEvent::Pong(ref pid, _) => PeerManagerEvent::PeerAlive { pid: pid.clone() },
                PingEvent::Timeout(ref pid) => {
                    let kind = MisbehaviorKind::PingTimeout;

                    PeerManagerEvent::Misbehave {
                        pid: pid.clone(),
                        kind,
                    }
                }
                PingEvent::UnexpectedError(ref pid) => {
                    let kind = MisbehaviorKind::PingUnexpect;

                    PeerManagerEvent::Misbehave {
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
