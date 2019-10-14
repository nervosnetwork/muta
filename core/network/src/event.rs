use std::error::Error;

use derive_more::{Display, From};
use futures::channel::oneshot::Sender;
use protocol::{traits::Priority, types::UserAddress};
use tentacle::{
    bytes::Bytes,
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
    service::{DialProtocol, SessionType, TargetSession},
    ProtocolId, SessionId,
};

#[derive(Debug, Display)]
pub enum ConnectionEvent {
    #[display(fmt = "connect addrs {:?}, proto: {:?}", addrs, proto)]
    Connect {
        addrs: Vec<Multiaddr>,
        proto: DialProtocol,
    },

    #[display(fmt = "disconnect session {}", _0)]
    Disconnect(SessionId),

    #[display(fmt = "send message to {:?}", tar)]
    SendMsg {
        tar: TargetSession,
        msg: Bytes,
        pri: Priority,
    },
}

#[derive(Debug, Display, From)]
pub enum RetryKind {
    #[display(fmt = "peer connection timeout")]
    TimedOut,

    #[display(fmt = "peer connection interrupted")]
    Interrupted,

    #[display(fmt = "peer protocol select failure, unstable connection")]
    ProtocolSelect,

    #[display(fmt = "peer multiplex stream error: {}", _0)]
    Multiplex(Box<dyn Error + Send>),

    // FIXME
    #[allow(dead_code)]
    #[display(fmt = "peer session closed")]
    SessionClosed,

    #[display(fmt = "{}", _0)]
    Other(&'static str),
}

#[derive(Debug, Display, From)]
pub enum RemoveKind {
    #[display(fmt = "unable to connect peer address {}: {}", addr, err)]
    UnableToConnect {
        addr: Multiaddr,
        err:  Box<dyn Error + Send>,
    },

    #[display(fmt = "unknown protocol {}", _0)]
    UnknownProtocol(String),

    #[display(fmt = "broken protocol {}: {}", proto_id, err)]
    BrokenProtocol {
        proto_id: ProtocolId,
        err:      Box<dyn Error + Send>,
    },

    #[display(fmt = "protocol blocked on peer {:?} session {}", pid, sid)]
    SessionBlocked { pid: PeerId, sid: SessionId },

    #[display(fmt = "bad session peer: {}", _0)]
    BadSessionPeer(String),
}

#[derive(Debug, Display)]
#[display(fmt = "multi users message, addrs: {:?}", user_addrs)]
pub struct MultiUsersMessage {
    pub user_addrs: Vec<UserAddress>,
    pub msg:        Bytes,
    pub pri:        Priority,
}

#[derive(Debug, Display, PartialEq, Eq)]
pub enum ConnectionType {
    #[display(fmt = "Receive an repeated connection")]
    Listen,
    #[display(fmt = "Dial an repeated connection")]
    Dialer,
}

#[derive(Debug, Display)]
#[display(fmt = "session {:?} addr {:?} ty {:?}", sid, addr, ty)]
pub struct Session {
    pub sid:  SessionId,
    pub addr: Multiaddr,
    pub ty:   SessionType,
}

#[derive(Debug, Display)]
pub enum PeerManagerEvent {
    // Peer
    #[display(fmt = "attach peer session {}", session)]
    AttachPeerSession {
        pubkey:  PublicKey,
        session: Session,
    },

    #[display(fmt = "detach peer {:?} session {:?} ty {:?}", pid, sid, ty)]
    DetachPeerSession {
        pid: PeerId,
        sid: SessionId,
        ty:  SessionType,
    },

    #[display(fmt = "peer {:?} alive", pid)]
    PeerAlive { pid: PeerId },

    #[display(fmt = "remove peer {:?} kind: {}", pid, kind)]
    RemovePeer { pid: PeerId, kind: RemoveKind },

    #[display(fmt = "remove peer by session {} kind: {}", sid, kind)]
    RemovePeerBySession { sid: SessionId, kind: RemoveKind },

    #[display(fmt = "retry peer {:?} later, disconnect now, kind: {}", pid, kind)]
    RetryPeerLater { pid: PeerId, kind: RetryKind },

    // Address
    #[display(fmt = "discover addr {}", addr)]
    DiscoverAddr { addr: Multiaddr },

    #[display(fmt = "discover multi addrs {:?}", addrs)]
    DiscoverMultiAddrs { addrs: Vec<Multiaddr> },

    #[display(fmt = "identify pid {:?} addrs {:?}", pid, addrs)]
    IdentifiedAddrs {
        pid:   PeerId,
        addrs: Vec<Multiaddr>,
    },

    #[display(fmt = "repeated connection type {} session {} addr {}", ty, sid, addr)]
    RepeatedConnection {
        ty:   ConnectionType,
        sid:  SessionId,
        addr: Multiaddr,
    },

    #[display(fmt = "unconnectable addr {}, kind: {}", addr, kind)]
    UnconnectableAddress { addr: Multiaddr, kind: RemoveKind },

    #[display(fmt = "re-connect later, addr {}, kind: {}", addr, kind)]
    ReconnectLater { addr: Multiaddr, kind: RetryKind },

    // Self
    #[display(fmt = "add listen addr {}", addr)]
    AddListenAddr { addr: Multiaddr },

    #[display(fmt = "rmeove listen addr {}", addr)]
    RemoveListenAddr { addr: Multiaddr },

    // Account addresses
    #[display(fmt = "try route multi accounts message: {}", users_msg)]
    RouteMultiUsersMessage {
        users_msg: MultiUsersMessage,
        miss_tx:   Sender<Vec<UserAddress>>,
    },
}
