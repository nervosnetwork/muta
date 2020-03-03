use std::{error::Error, sync::Arc};

use derive_more::Display;
use protocol::types::Address;
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

#[derive(Debug, Display)]
pub enum ProtocolIdentity {
    #[display(fmt = "protocol id {}", _0)]
    Id(ProtocolId),
    #[display(fmt = "protocol name {}", _0)]
    Name(String),
}

#[derive(Debug, Display)]
pub enum ConnectionErrorKind {
    #[display(fmt = "io {:?}", _0)]
    Io(std::io::Error),

    #[display(fmt = "dns resolver {}", _0)]
    DNSResolver(Box<dyn Error + Send>),

    #[display(fmt = "handshake {}", _0)]
    SecioHandshake(Box<dyn Error + Send>),

    #[display(fmt = "remote peer doesn't match one in multiaddr")]
    PeerIdNotMatch,

    #[display(fmt = "protocol handle block or abnormally closed")]
    ProtocolHandle,
}

#[derive(Debug, Display)]
pub enum SessionErrorKind {
    #[display(fmt = "io {:?}", _0)]
    Io(std::io::Error),

    // Maybe unknown protocol, protocol version incompatible, protocol codec
    // error
    #[display(fmt = "protocol identity {:?} {:?}", identity, cause)]
    Protocol {
        identity: Option<ProtocolIdentity>,
        cause:    Option<Box<dyn Error + Send>>,
    },

    #[display(fmt = "unexpect {}", _0)]
    Unexpected(Box<dyn Error + Send>),
}

#[derive(Debug, Display)]
pub enum MisbehaviorKind {
    #[display(fmt = "discovery")]
    Discovery,

    #[display(fmt = "ping time out")]
    PingTimeout,

    // Maybe message codec or nonce incorrect
    #[display(fmt = "ping unexpect")]
    PingUnexpect,
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
    #[display(fmt = "connect peers {:?} now", pids)]
    ConnectPeersNow { pids: Vec<PeerId> },

    #[display(fmt = "connect to {} failed, kind: {}", addr, kind)]
    ConnectFailed {
        addr: Multiaddr,
        kind: ConnectionErrorKind,
    },

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

    #[display(fmt = "repeated connection type {} session {} addr {}", ty, sid, addr)]
    RepeatedConnection {
        ty:   ConnectionType,
        sid:  SessionId,
        addr: Multiaddr,
    },

    #[display(
        fmt = "session {} blocked, pending data size {}",
        "ctx.id",
        "ctx.pending_data_size()"
    )]
    SessionBlocked { ctx: Arc<SessionContext> },

    #[display(fmt = "peer {:?} session {} closed", pid, sid)]
    SessionClosed { pid: PeerId, sid: SessionId },

    #[display(fmt = "session {} failed, kind: {}", sid, kind)]
    SessionFailed {
        sid:  SessionId,
        kind: SessionErrorKind,
    },

    #[display(fmt = "peer {:?} alive", pid)]
    PeerAlive { pid: PeerId },

    #[display(fmt = "peer {:?} misbehave {}", pid, kind)]
    Misbehave { pid: PeerId, kind: MisbehaviorKind },

    #[display(fmt = "protect peers by chain addresses {:?}", chain_addrs)]
    ProtectPeersByChainAddr { chain_addrs: Vec<Address> },

    // Address
    #[display(fmt = "discover multi addrs {:?}", addrs)]
    DiscoverMultiAddrs { addrs: Vec<Multiaddr> },

    #[display(fmt = "identify pid {:?} addrs {:?}", pid, addrs)]
    IdentifiedAddrs {
        pid:   PeerId,
        addrs: Vec<Multiaddr>,
    },

    // Self
    #[display(fmt = "add listen addr {}", addr)]
    AddNewListenAddr { addr: Multiaddr },

    #[display(fmt = "rmeove listen addr {}", addr)]
    RemoveListenAddr { addr: Multiaddr },
}
