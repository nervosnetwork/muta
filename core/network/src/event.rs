use std::{error::Error, sync::Arc};

use derive_more::{Display, From};
use protocol::{traits::Priority, types::Address, Bytes};
#[cfg(not(test))]
use tentacle::context::SessionContext;
use tentacle::{
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
    service::TargetProtocol,
    ProtocolId, SessionId,
};

#[cfg(test)]
use crate::test::mock::SessionContext;

#[derive(Debug, Display)]
pub enum ConnectionEvent {
    #[display(fmt = "connect addrs {:?}, proto: {:?}", addrs, proto)]
    Connect {
        addrs: Vec<Multiaddr>,
        proto: TargetProtocol,
    },

    #[display(fmt = "disconnect session {}", _0)]
    Disconnect(SessionId),
}

#[derive(Debug, Display, From)]
pub enum RetryKind {
    #[display(fmt = "peer connection timeout")]
    TimedOut,

    #[display(fmt = "peer {:?}", _0)]
    Io(std::io::ErrorKind),

    #[display(fmt = "peer multiplex stream error: {}", _0)]
    Multiplex(Box<dyn Error + Send>),

    // FIXME
    #[allow(dead_code)]
    #[display(fmt = "peer session closed")]
    SessionClosed,

    #[display(fmt = "{}", _0)]
    Other(&'static str),
}

#[derive(Debug, Display)]
pub enum RemoveKind {
    #[display(fmt = "unable to connect peer address {}: {}", addr, err)]
    UnableToConnect {
        addr: Multiaddr,
        err:  Box<dyn Error + Send>,
    },

    #[display(fmt = "unknown protocol {}", _0)]
    UnknownProtocol(String),

    #[display(fmt = "protocol select")]
    ProtocolSelect,

    #[display(fmt = "broken protocol {}: {}", proto_id, err)]
    BrokenProtocol {
        proto_id: ProtocolId,
        err:      Box<dyn Error + Send>,
    },

    #[display(fmt = "bad session peer: {}", _0)]
    BadSessionPeer(String),
}

#[derive(Debug, Display)]
#[display(fmt = "multi users message, addrs: {:?}", user_addrs)]
pub struct MultiUsersMessage {
    pub user_addrs: Vec<Address>,
    pub msg:        Bytes,
    pub pri:        Priority,
}

#[derive(Debug, Display, PartialEq, Eq)]
pub enum ConnectionType {
    #[display(fmt = "Receive an repeated connection")]
    Inbound,
    #[display(fmt = "Dial an repeated connection")]
    Outbound,
}

#[derive(Debug, Display)]
pub enum PeerManagerEvent {
    // Peer
    #[display(
        fmt = "new session {} peer {:?} addr {} ty {:?}",
        "ctx.id",
        pid,
        "ctx.address",
        "ctx.ty"
    )]
    NewSession {
        pid:    PeerId,
        pubkey: PublicKey,
        ctx:    Arc<SessionContext>,
    },

    #[display(fmt = "peer {:?} session {} closed", pid, sid)]
    SessionClosed { pid: PeerId, sid: SessionId },

    #[display(fmt = "peer {:?} alive", pid)]
    PeerAlive { pid: PeerId },

    #[display(
        fmt = "session {} blocked, pending data size {}",
        "ctx.id",
        "ctx.pending_data_size()"
    )]
    SessionBlocked { ctx: Arc<SessionContext> },

    #[display(fmt = "remove peer by session {} kind: {}", sid, kind)]
    RemovePeerBySession { sid: SessionId, kind: RemoveKind },

    #[display(fmt = "retry peer {:?} later, disconnect now, kind: {}", pid, kind)]
    RetryPeerLater { pid: PeerId, kind: RetryKind },

    #[display(fmt = "connect peers {:?} now", pids)]
    ConnectPeersNow { pids: Vec<PeerId> },

    #[display(fmt = "protect peers by chain addresses {:?}", chain_addrs)]
    ProtectPeersByChainAddr { chain_addrs: Vec<Address> },

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

    #[display(fmt = "reconnect later, addr {}, kind: {}", addr, kind)]
    ReconnectAddrLater { addr: Multiaddr, kind: RetryKind },

    // Self
    #[display(fmt = "add listen addr {}", addr)]
    AddNewListenAddr { addr: Multiaddr },

    #[display(fmt = "rmeove listen addr {}", addr)]
    RemoveListenAddr { addr: Multiaddr },
}
