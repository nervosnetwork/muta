use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use derive_more::Display;
use parking_lot::RwLock;
use tentacle::service::SessionType;
use tentacle::SessionId;

use super::{ArcPeer, PeerManagerConfig};
use crate::common::ConnectedAddr;
use crate::config::{
    DEFAULT_INBOUND_CONN_LIMIT, DEFAULT_MAX_CONNECTIONS, DEFAULT_SAME_IP_CONN_LIMIT,
};

#[cfg(test)]
pub use crate::test::mock::SessionContext;
#[cfg(not(test))]
pub use tentacle::context::SessionContext;

type Host = String;
type Count = usize;

#[derive(Debug, Display, PartialEq, Eq)]
pub enum Error {
    #[display(fmt = "reach same ip connections limit")]
    ReachSameIPConnLimit,

    #[display(fmt = "reach inbound connections limit")]
    ReachInboundConnLimit,

    #[display(fmt = "reach outbound connections limit")]
    ReachOutboundConnLimit,
}

#[derive(Debug)]
pub struct Config {
    same_ip_conn_limit:  usize,
    inbound_conn_limit:  usize,
    outbound_conn_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            same_ip_conn_limit:  DEFAULT_SAME_IP_CONN_LIMIT,
            inbound_conn_limit:  DEFAULT_INBOUND_CONN_LIMIT,
            outbound_conn_limit: DEFAULT_MAX_CONNECTIONS - DEFAULT_INBOUND_CONN_LIMIT,
        }
    }
}

impl From<&PeerManagerConfig> for Config {
    fn from(config: &PeerManagerConfig) -> Config {
        Config {
            same_ip_conn_limit:  config.same_ip_conn_limit,
            inbound_conn_limit:  config.inbound_conn_limit,
            outbound_conn_limit: config.outbound_conn_limit,
        }
    }
}

#[derive(Debug)]
pub struct Session {
    pub(crate) id:             SessionId,
    pub(crate) ctx:            Arc<SessionContext>,
    pub(crate) peer:           ArcPeer,
    blocked:                   AtomicBool,
    pub(crate) connected_addr: ConnectedAddr,
}

#[derive(Debug, Clone)]
pub struct ArcSession(Arc<Session>);

impl ArcSession {
    pub fn new(peer: ArcPeer, ctx: Arc<SessionContext>) -> Self {
        let connected_addr = ConnectedAddr::from(&ctx.address);
        let session = Session {
            id: ctx.id,
            ctx,
            peer,
            blocked: AtomicBool::new(false),
            connected_addr,
        };

        ArcSession(Arc::new(session))
    }

    pub fn ty(&self) -> SessionType {
        self.ctx.ty
    }

    pub fn block(&self) {
        self.blocked.store(true, Ordering::SeqCst);
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked.load(Ordering::SeqCst)
    }

    pub fn unblock(&self) {
        self.blocked.store(false, Ordering::SeqCst);
    }
}

impl Borrow<SessionId> for ArcSession {
    fn borrow(&self) -> &SessionId {
        &self.id
    }
}

impl PartialEq for ArcSession {
    fn eq(&self, other: &ArcSession) -> bool {
        self.id == other.id
    }
}

impl Eq for ArcSession {}

impl Hash for ArcSession {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl Deref for ArcSession {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct AcceptableSession(pub ArcSession);

pub struct SessionBook {
    config: Config,

    hosts:    RwLock<HashMap<Host, Count>>,
    sessions: RwLock<HashSet<ArcSession>>,

    inbound_count:  AtomicUsize,
    outbound_count: AtomicUsize,
}

impl Default for SessionBook {
    fn default() -> SessionBook {
        let config = Config::default();

        SessionBook::new(config)
    }
}

impl SessionBook {
    pub fn new(config: Config) -> Self {
        SessionBook {
            config,
            hosts: Default::default(),
            sessions: Default::default(),
            inbound_count: AtomicUsize::new(0),
            outbound_count: AtomicUsize::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.sessions.read().len()
    }

    pub fn get(&self, sid: &SessionId) -> Option<ArcSession> {
        self.sessions.read().get(sid).cloned()
    }

    pub fn all(&self) -> Vec<ArcSession> {
        self.sessions.read().iter().cloned().collect()
    }

    pub fn iter_fn<R, F>(&self, f: F) -> R
    where
        F: for<'a> FnOnce(&mut dyn Iterator<Item = &'a ArcSession>) -> R,
    {
        let sessions = self.sessions.read();
        f(&mut sessions.iter())
    }

    pub fn inbound_count(&self) -> usize {
        self.inbound_count.load(Ordering::SeqCst)
    }

    pub fn outbound_count(&self) -> usize {
        self.outbound_count.load(Ordering::SeqCst)
    }

    pub fn acceptable(&self, session: &ArcSession) -> Result<(), self::Error> {
        let session_host = &session.connected_addr.host;
        let host_count = {
            let hosts = self.hosts.read();
            hosts.get(session_host).cloned().unwrap_or(0)
        };

        if host_count == usize::MAX || host_count + 1 > self.config.same_ip_conn_limit {
            return Err(self::Error::ReachSameIPConnLimit);
        }

        match session.ty() {
            SessionType::Inbound if self.inbound_count() >= self.config.inbound_conn_limit => {
                Err(self::Error::ReachInboundConnLimit)
            }
            SessionType::Outbound if self.outbound_count() >= self.config.outbound_conn_limit => {
                Err(self::Error::ReachOutboundConnLimit)
            }
            _ => Ok(()),
        }
    }

    pub fn insert(&self, AcceptableSession(session): AcceptableSession) {
        let session_host = &session.connected_addr.host;

        let mut hosts = self.hosts.write();
        hosts
            .entry(session_host.to_owned())
            .and_modify(|c| *c += 1)
            .or_insert(1);

        match session.ty() {
            SessionType::Inbound => self.inbound_count.fetch_add(1, Ordering::SeqCst),
            SessionType::Outbound => self.outbound_count.fetch_add(1, Ordering::SeqCst),
        };

        self.sessions.write().insert(session);
    }

    pub fn remove(&self, sid: &SessionId) -> Option<ArcSession> {
        let session = self.sessions.write().take(sid);

        if let Some(connected_addr) = session.as_ref().map(|s| &s.connected_addr) {
            let session_host = &connected_addr.host;
            let mut hosts = self.hosts.write();

            if hosts.get(session_host) == Some(&1) {
                hosts.remove(session_host);
            } else if let Some(count) = hosts.get_mut(session_host) {
                *count -= 1;
            }
        }

        if let Some(ty) = session.as_ref().map(|s| s.ty()) {
            match ty {
                SessionType::Inbound => self.inbound_count.fetch_sub(1, Ordering::SeqCst),
                SessionType::Outbound => self.outbound_count.fetch_sub(1, Ordering::SeqCst),
            };
        }

        session
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::sync::Arc;

    use tentacle::multiaddr::Multiaddr;
    use tentacle::secio::{PeerId, SecioKeyPair};
    use tentacle::service::SessionType;
    use tentacle::SessionId;

    use super::{AcceptableSession, ArcSession, Config, Error, SessionBook};
    use crate::peer_manager::{ArcPeer, PeerMultiaddr};
    use crate::test::mock::SessionContext;
    use crate::traits::MultiaddrExt;

    fn make_multiaddr(port: u16, id: Option<PeerId>) -> Multiaddr {
        let mut multiaddr = format!("/ip4/127.0.0.1/tcp/{}", port)
            .parse::<Multiaddr>()
            .expect("peer multiaddr");

        if let Some(id) = id {
            multiaddr.push_id(id);
        }

        multiaddr
    }

    fn make_peer_multiaddr(port: u16, id: PeerId) -> PeerMultiaddr {
        make_multiaddr(port, Some(id))
            .try_into()
            .expect("try into peer multiaddr")
    }

    fn make_peer(port: u16) -> ArcPeer {
        let keypair = SecioKeyPair::secp256k1_generated();
        let pubkey = keypair.public_key();
        let peer_id = pubkey.peer_id();
        let peer = ArcPeer::from_pubkey(pubkey).expect("make peer");
        let multiaddr = make_peer_multiaddr(port, peer_id);

        peer.multiaddrs.set(vec![multiaddr]);
        peer
    }

    fn make_session(port: u16, sid: SessionId, ty: SessionType) -> ArcSession {
        let peer = make_peer(port);
        let multiaddr = peer.multiaddrs.all_raw().pop().unwrap();
        let ctx = SessionContext::make(sid, multiaddr, ty, peer.owned_pubkey().unwrap());

        ArcSession::new(peer, Arc::new(ctx))
    }

    #[test]
    fn should_reject_session_when_reach_same_ip_conn_limit() {
        let config = Config {
            same_ip_conn_limit:  1,
            inbound_conn_limit:  20,
            outbound_conn_limit: 20,
        };
        let book = SessionBook::new(config);

        let session = make_session(100, 1.into(), SessionType::Inbound);
        assert!(book.acceptable(&session).is_ok());

        book.insert(AcceptableSession(session.clone()));
        assert_eq!(
            book.hosts.read().get(&session.connected_addr.host),
            Some(&1)
        );

        let same_ip_session = make_session(101, 2.into(), SessionType::Inbound);
        assert_eq!(
            book.acceptable(&same_ip_session),
            Err(Error::ReachSameIPConnLimit)
        );
    }

    #[test]
    fn should_reduce_host_count() {
        let config = Config {
            same_ip_conn_limit:  5,
            inbound_conn_limit:  20,
            outbound_conn_limit: 20,
        };
        let book = SessionBook::new(config);

        let session = make_session(100, 1.into(), SessionType::Inbound);
        assert!(book.acceptable(&session).is_ok());

        book.insert(AcceptableSession(session.clone()));
        assert_eq!(
            book.hosts.read().get(&session.connected_addr.host),
            Some(&1)
        );

        book.remove(&(1.into()));
        assert_eq!(book.hosts.read().get(&session.connected_addr.host), None);
    }

    #[test]
    fn should_reject_inbound_session_when_reach_inbound_limit() {
        let config = Config {
            same_ip_conn_limit:  5,
            inbound_conn_limit:  1,
            outbound_conn_limit: 20,
        };
        let book = SessionBook::new(config);

        let session = make_session(100, 1.into(), SessionType::Inbound);
        assert!(book.acceptable(&session).is_ok());

        book.insert(AcceptableSession(session.clone()));
        assert_eq!(
            book.hosts.read().get(&session.connected_addr.host),
            Some(&1)
        );
        assert_eq!(book.inbound_count(), 1);

        let same_ip_session = make_session(101, 2.into(), SessionType::Inbound);
        assert_eq!(
            book.acceptable(&same_ip_session),
            Err(Error::ReachInboundConnLimit)
        );
    }

    #[test]
    fn should_reject_outbound_session_when_reach_outbound_limit() {
        let config = Config {
            same_ip_conn_limit:  5,
            inbound_conn_limit:  10,
            outbound_conn_limit: 1,
        };
        let book = SessionBook::new(config);

        let session = make_session(100, 1.into(), SessionType::Outbound);
        assert!(book.acceptable(&session).is_ok());

        book.insert(AcceptableSession(session.clone()));
        assert_eq!(
            book.hosts.read().get(&session.connected_addr.host),
            Some(&1)
        );
        assert_eq!(book.outbound_count(), 1);

        let same_ip_session = make_session(101, 2.into(), SessionType::Outbound);
        assert_eq!(
            book.acceptable(&same_ip_session),
            Err(Error::ReachOutboundConnLimit)
        );
    }
}
