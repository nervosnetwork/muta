use super::{Inner, TrustMetric, WORSE_TRUST_SCALAR_RATIO};

use derive_more::Display;
use protocol::types::Address;
use tentacle::SessionId;

use std::sync::Arc;

pub type GoodEvents = usize;
pub type BadEvents = usize;

#[derive(Debug, Display)]
#[display(fmt = "not found")]
pub struct NotFound {}
impl std::error::Error for NotFound {}

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

    pub fn trust_metric_wrose_scalar_ratio(&self) -> usize {
        WORSE_TRUST_SCALAR_RATIO
    }

    pub fn session_trust_score(&self, sid: SessionId) -> Option<u8> {
        self.session_trust_metric(sid)
            .map(|metric| metric.trust_score())
    }

    pub fn session_trust_events(&self, sid: SessionId) -> Option<(GoodEvents, BadEvents)> {
        self.session_trust_metric(sid).map(|metric| metric.events())
    }

    pub fn session_trust_force_new_interval(&self, sid: SessionId) -> Result<(), NotFound> {
        let trust_metric = self.session_trust_metric(sid).ok_or_else(|| NotFound {})?;

        trust_metric.enter_new_interval();
        Ok(())
    }

    fn session_trust_metric(&self, sid: SessionId) -> Option<TrustMetric> {
        self.0
            .session(sid)
            .map(|sess| sess.peer.trust_metric())
            .flatten()
    }
}
