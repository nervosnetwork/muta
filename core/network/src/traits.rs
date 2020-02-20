use std::borrow::Cow;

use async_trait::async_trait;
use protocol::{
    traits::{Context, Priority},
    types::Address,
    Bytes,
};
use tentacle::{
    multiaddr::Multiaddr,
    secio::PeerId,
    service::TargetSession,
    service::{ProtocolMeta, TargetProtocol},
    ProtocolId, SessionId,
};

use crate::{
    common::ConnectedAddr,
    error::{ErrorKind, NetworkError},
};

pub trait NetworkProtocol {
    // TODO: change to TargetProtocol after tentacle 0.3
    fn target() -> TargetProtocol;

    fn metas(self) -> Vec<ProtocolMeta>;

    fn message_proto_id() -> ProtocolId;
}

#[rustfmt::skip]
#[async_trait]
pub trait MessageSender {
    fn send(&self, tar: TargetSession, msg: Bytes, pri: Priority) -> Result<(), NetworkError>;
    async fn users_send(&self, users: Vec<Address>, msg: Bytes, pri: Priority) -> Result<(), NetworkError>;
}

pub trait Compression {
    fn compress(&self, bytes: Bytes) -> Result<Bytes, NetworkError>;
    fn decompress(&self, bytes: Bytes) -> Result<Bytes, NetworkError>;
}

pub trait NetworkContext: Sized {
    fn session_id(&self) -> Result<SessionId, NetworkError>;
    fn set_session_id(&mut self, sid: SessionId) -> Self;
    fn remote_peer_id(&self) -> Result<PeerId, NetworkError>;
    fn set_remote_peer_id(&mut self, pid: PeerId) -> Self;
    // This connected address is for debug purpose, so soft failure is ok.
    fn remote_connected_addr(&self) -> Option<ConnectedAddr>;
    fn set_remote_connected_addr(&mut self, addr: ConnectedAddr) -> Self;
    fn rpc_id(&self) -> Result<u64, NetworkError>;
    fn set_rpc_id(&mut self, rid: u64) -> Self;
}

pub trait ListenExchangeManager {
    fn listen_addr(&self) -> Multiaddr;
    fn add_remote_listen_addr(&mut self, pid: PeerId, addr: Multiaddr);
    fn misbehave(&mut self, sid: SessionId);
}

pub trait SessionBook {
    fn all_sendable(&self) -> Vec<SessionId>;
    fn all_blocked(&self) -> Vec<SessionId>;
    fn refresh_blocked(&self);
    fn by_chain(&self, addrs: Vec<Address>) -> (Vec<SessionId>, Vec<Address>);
    fn peers_by_chain(&self, addrs: Vec<Address>) -> (Vec<PeerId>, Vec<Address>);
    fn peers(&self) -> Vec<PeerId>;
    fn connected_addr(&self, pid: &PeerId) -> Option<ConnectedAddr>;
    fn pending_data_size(&self, pid: &PeerId) -> usize;
}

pub trait MultiaddrExt {
    fn peer_id_bytes(&self) -> Option<Cow<'_, [u8]>>;
    fn has_peer_id(&self) -> bool;
    fn push_id(&mut self, peer_id: PeerId);
}

#[derive(Debug, Clone)]
struct CtxSessionId(SessionId);

#[derive(Debug, Clone)]
struct CtxRpcId(u64);

impl NetworkContext for Context {
    fn session_id(&self) -> Result<SessionId, NetworkError> {
        self.get::<CtxSessionId>("session_id")
            .map(|ctx_sid| ctx_sid.0)
            .ok_or_else(|| ErrorKind::NoSessionId.into())
    }

    #[must_use]
    fn set_session_id(&mut self, sid: SessionId) -> Self {
        self.with_value::<CtxSessionId>("session_id", CtxSessionId(sid))
    }

    fn remote_peer_id(&self) -> Result<PeerId, NetworkError> {
        self.get::<PeerId>("remote_peer_id")
            .map(ToOwned::to_owned)
            .ok_or_else(|| ErrorKind::NoRemotePeerId.into())
    }

    #[must_use]
    fn set_remote_peer_id(&mut self, pid: PeerId) -> Self {
        self.with_value::<PeerId>("remote_peer_id", pid)
    }

    fn remote_connected_addr(&self) -> Option<ConnectedAddr> {
        self.get::<ConnectedAddr>("remote_connected_addr")
            .map(ToOwned::to_owned)
    }

    #[must_use]
    fn set_remote_connected_addr(&mut self, addr: ConnectedAddr) -> Self {
        self.with_value::<ConnectedAddr>("remote_connected_addr", addr)
    }

    fn rpc_id(&self) -> Result<u64, NetworkError> {
        self.get::<CtxRpcId>("rpc_id")
            .map(|ctx_rid| ctx_rid.0)
            .ok_or_else(|| ErrorKind::NoRpcId.into())
    }

    #[must_use]
    fn set_rpc_id(&mut self, rid: u64) -> Self {
        self.with_value::<CtxRpcId>("rpc_id", CtxRpcId(rid))
    }
}
