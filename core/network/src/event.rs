use std::error::Error;

use derive_more::{Display, From};
use futures::channel::oneshot::Sender;
use protocol::{traits::Priority, types::UserAddress};
use tentacle::{
    bytes::Bytes,
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
    service::{DialProtocol, TargetSession},
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

#[derive(Debug, Display)]
pub enum PeerManagerEvent {
    // Peer
    #[display(fmt = "add peer {:?} addr {}", pid, addr)]
    AddPeer {
        pid:    PeerId,
        pubkey: PublicKey,
        addr:   Multiaddr,
    },

    #[display(fmt = "update peer {:?} session {:?}", pid, sid)]
    UpdatePeerSession { pid: PeerId, sid: Option<SessionId> },

    #[display(fmt = "peer {:?} alive", pid)]
    PeerAlive { pid: PeerId },

    #[display(fmt = "remove peer {:?} kind: {}", pid, kind)]
    RemovePeer { pid: PeerId, kind: RemoveKind },

    #[display(fmt = "remove peer by session {} kind: {}", sid, kind)]
    RemovePeerBySession { sid: SessionId, kind: RemoveKind },

    #[display(fmt = "retry peer {:?} later, disconnect now, kind: {}", pid, kind)]
    RetryPeerLater { pid: PeerId, kind: RetryKind },

    // Address
    #[display(fmt = "add unknown addr {}", addr)]
    AddUnknownAddr { addr: Multiaddr },

    #[display(fmt = "add multi unknown addrs {:?}", addrs)]
    AddMultiUnknownAddrs { addrs: Vec<Multiaddr> },

    // FIXME
    #[allow(dead_code)]
    #[display(fmt = "add session {} addr {}", sid, addr)]
    AddSessionAddr { sid: SessionId, addr: Multiaddr },

    // FIXME
    #[allow(dead_code)]
    #[display(fmt = "add session {} multi addrs {:?}", sid, addrs)]
    AddSessionMultiAddrs {
        sid:   SessionId,
        addrs: Vec<Multiaddr>,
    },

    #[display(fmt = "remove unknown addr {}, kind: {}", addr, kind)]
    RemoveAddr { addr: Multiaddr, kind: RemoveKind },

    #[display(fmt = "retry unknown addr {}, kind: {}", addr, kind)]
    RetryAddrLater { addr: Multiaddr, kind: RetryKind },

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
