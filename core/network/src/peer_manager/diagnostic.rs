use super::{Inner, WORSE_TRUST_SCALAR_RATIO};
use crate::event::PeerManagerEvent;

use derive_more::Display;
use protocol::{traits::TrustFeedback, types::Address};
use tentacle::SessionId;

use std::sync::Arc;

#[derive(Debug, Display)]
#[display(fmt = "not found")]
pub struct NotFound {}
impl std::error::Error for NotFound {}

#[derive(Debug, Display, Clone)]
pub enum DiagnosticEvent {
    #[display(fmt = "new session")]
    NewSession,

    #[display(fmt = "session closed")]
    SessionClosed,

    #[display(fmt = "trust metric feedback {}", feedback)]
    TrustMetric { feedback: TrustFeedback },

    #[display(fmt = "trust new interval report {}", report)]
    TrustNewInterval { report: TrustReport },

    #[display(fmt = "remote height {}", height)]
    RemoteHeight { height: u64 },
}

impl From<&PeerManagerEvent> for Option<DiagnosticEvent> {
    fn from(event: &PeerManagerEvent) -> Self {
        use PeerManagerEvent::{NewSession, SessionClosed, TrustMetric};

        match event {
            NewSession { .. } => Some(DiagnosticEvent::NewSession),
            SessionClosed { .. } => Some(DiagnosticEvent::SessionClosed),
            TrustMetric { feedback, .. } => Some(DiagnosticEvent::TrustMetric {
                feedback: feedback.to_owned(),
            }),
            _ => None,
        }
    }
}

pub type DiagnosticHookFn = Box<dyn Fn(DiagnosticEvent) + Send + 'static>;

#[derive(Debug, Display, Clone, Copy)]
#[display(
    fmt = "score {}, good {}, bad {}, worse scalar ratio {}",
    score,
    bad_events,
    good_events,
    worse_scalar_ratio
)]
pub struct TrustReport {
    pub score:              u8,
    pub bad_events:         usize,
    pub good_events:        usize,
    pub worse_scalar_ratio: usize,
}

#[derive(Clone)]
pub struct Diagnostic(Arc<Inner>);

impl Diagnostic {
    pub(super) fn new(inner: Arc<Inner>) -> Self {
        Diagnostic(inner)
    }

    pub fn session_by_chain(&self, addr: &Address) -> Option<SessionId> {
        let chain = self.0.chain.read();

        match chain.get(addr).map(|peer| peer.session_id()) {
            Some(sid) if sid != SessionId::new(0) => Some(sid),
            _ => None,
        }
    }

    pub fn new_trust_interval(&self, sid: SessionId) -> Result<TrustReport, NotFound> {
        let session = self.0.session(sid).ok_or_else(|| NotFound {})?;
        let metric = session.peer.trust_metric().ok_or_else(|| NotFound {})?;

        let score = metric.trust_score();
        let (good_events, bad_events) = metric.events();
        let report = TrustReport {
            score,
            good_events,
            bad_events,
            worse_scalar_ratio: WORSE_TRUST_SCALAR_RATIO,
        };

        metric.enter_new_interval();
        Ok(report)
    }
}
