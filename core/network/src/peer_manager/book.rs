use super::{ArcSession, Inner, Peer};
use crate::traits::SessionBook;

use parking_lot::RwLock;
use protocol::types::Address;
use tentacle::{multiaddr::Multiaddr, SessionId};

use std::{collections::HashSet, sync::Arc};

pub struct SharedSessionsConfig {
    pub max_stream_window_size: usize,
    pub write_timeout:          u64,
}

#[derive(Clone)]
pub struct SharedSessions {
    inner:  Arc<Inner>,
    config: Arc<SharedSessionsConfig>,
}

impl SharedSessions {
    pub(super) fn new(inner: Arc<Inner>, config: SharedSessionsConfig) -> Self {
        SharedSessions {
            inner,
            config: Arc::new(config),
        }
    }

    fn sessions(&self) -> &RwLock<HashSet<ArcSession>> {
        &self.inner.sessions
    }
}

impl SessionBook for SharedSessions {
    fn all_sendable(&self) -> Vec<SessionId> {
        self.sessions()
            .read()
            .iter()
            .filter(|s| !s.is_blocked())
            .map(|s| s.id)
            .collect()
    }

    fn all_blocked(&self) -> Vec<SessionId> {
        self.sessions()
            .read()
            .iter()
            .filter(|s| s.is_blocked())
            .map(|s| s.id)
            .collect()
    }

    fn refresh_blocked(&self) {
        let all_sessions = { self.sessions().read().clone() };

        for session in all_sessions {
            let pending_data_size = session.ctx.pending_data_size();
            // FIXME: multi streams
            let estimated_time = (pending_data_size / self.config.max_stream_window_size) as u64;

            if estimated_time < self.config.write_timeout {
                session.unblock()
            }
        }
    }

    fn by_chain(&self, addrs: Vec<Address>) -> (Vec<SessionId>, Vec<Address>) {
        let user_pid = self.inner.user_pid.read();
        let pool = self.inner.pool.read();

        let mut connected = Vec::new();
        let mut unconnected = Vec::new();
        for addr in addrs {
            if let Some(Some(Some(sid))) = user_pid
                .get(&addr)
                .map(|pid| pool.get(&pid).map(|p| p.session().map(|ctx| ctx.id)))
            {
                connected.push(sid);
            } else {
                unconnected.push(addr);
            }
        }

        (connected, unconnected)
    }

    fn multiaddrs(&self, addrs: Vec<Address>) -> (Vec<Multiaddr>, Vec<Address>) {
        let user_pid = self.inner.user_pid.read();
        let pool = self.inner.pool.read();

        let mut multiaddrs = Vec::new();
        let mut unknown = Vec::new();
        for addr in addrs {
            if let Some(Some(peer_addrs)) = user_pid
                .get(&addr)
                .map(|pid| pool.get(&pid).map(Peer::owned_addrs))
            {
                multiaddrs.extend(peer_addrs);
            } else {
                unknown.push(addr);
            }
        }

        (multiaddrs, unknown)
    }
}
