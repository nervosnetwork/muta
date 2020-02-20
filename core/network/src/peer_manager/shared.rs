use super::{ArcSession, Connectedness, Inner};
use crate::{common::ConnectedAddr, traits::SessionBook};

use log::debug;
use parking_lot::RwLock;
use protocol::types::Address;
use tentacle::{secio::PeerId, SessionId};

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
        let all_blocked = {
            self.sessions()
                .read()
                .iter()
                .filter(|s| s.is_blocked())
                .cloned()
                .collect::<Vec<_>>()
        };

        for session in all_blocked {
            let pending_data_size = session.ctx.pending_data_size();
            // FIXME: multi streams
            let estimated_time = (pending_data_size / self.config.max_stream_window_size) as u64;

            if estimated_time < self.config.write_timeout {
                debug!("unblock session {}", session.id);
                session.unblock()
            }
        }
    }

    fn by_chain(&self, addrs: Vec<Address>) -> (Vec<SessionId>, Vec<Address>) {
        let chain = self.inner.chain.read();

        let mut connected = Vec::new();
        let mut unconnected = Vec::new();
        for addr in addrs {
            if let Some(peer) = chain.get(&addr) {
                if peer.connectedness() == Connectedness::Connected {
                    connected.push(peer.session_id());
                } else {
                    unconnected.push(addr);
                }
            }
        }

        (connected, unconnected)
    }

    fn peers_by_chain(&self, addrs: Vec<Address>) -> (Vec<PeerId>, Vec<Address>) {
        let chain = self.inner.chain.read();

        let mut peers = Vec::new();
        let mut unknown = Vec::new();
        for addr in addrs {
            if let Some(peer) = chain.get(&addr) {
                peers.push(peer.id.as_ref().to_owned());
            } else {
                unknown.push(addr);
            }
        }

        (peers, unknown)
    }

    fn all(&self) -> Vec<SessionId> {
        self.sessions().read().iter().map(|s| s.id).collect()
    }

    fn connected_addr(&self, sid: SessionId) -> Option<ConnectedAddr> {
        self.sessions()
            .read()
            .get(&sid)
            .map(|s| s.connected_addr.to_owned())
    }

    fn pending_data_size(&self, sid: SessionId) -> usize {
        self.sessions()
            .read()
            .get(&sid)
            .map(|s| s.ctx.pending_data_size())
            .unwrap_or_else(|| 0)
    }
}
