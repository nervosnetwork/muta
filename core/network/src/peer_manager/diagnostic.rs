use super::Inner;

use protocol::types::Address;
use tentacle::SessionId;

use std::sync::Arc;

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
}
