use super::{ArcSession, Connectedness, Inner};
use crate::{common::ConnectedAddr, traits::SessionBook};

use log::debug;
use parking_lot::RwLock;
use protocol::traits::PeerTag;
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

    fn peers(&self, pids: Vec<PeerId>) -> (Vec<SessionId>, Vec<PeerId>) {
        let mut connected = Vec::new();
        let mut unconnected = Vec::new();

        for peer_id in pids {
            match self.inner.peer(&peer_id) {
                Some(peer) if peer.connectedness() == Connectedness::Connected => {
                    connected.push(peer.session_id())
                }
                _ => unconnected.push(peer_id),
            }
        }

        (connected, unconnected)
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

    fn allowlist(&self) -> Vec<PeerId> {
        self.sessions()
            .read()
            .iter()
            .filter_map(|s| {
                if s.peer.tags.contains(&PeerTag::AlwaysAllow) {
                    Some(s.peer.id.to_owned())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionBook, SharedSessions, SharedSessionsConfig};
    use crate::peer_manager::Inner;

    use tentacle::secio::SecioKeyPair;

    use std::sync::Arc;

    #[test]
    fn should_return_unconnected_peer_ids() {
        let sess_conf = SharedSessionsConfig {
            max_stream_window_size: 10,
            write_timeout:          10,
        };

        let inner = Arc::new(Inner::new());
        let sessions = SharedSessions::new(Arc::clone(&inner), sess_conf);

        let keypair = SecioKeyPair::secp256k1_generated();
        let pubkey = keypair.public_key();
        let peer_id = pubkey.peer_id();
        assert!(inner.peer(&peer_id).is_none(), "should not be registered");

        let (_, unconnected) = sessions.peers(vec![peer_id.clone()]);
        assert!(unconnected.contains(&peer_id));
    }
}
