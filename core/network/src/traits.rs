use protocol::traits::{Cloneable, Context, Priority};
use tentacle::{
    bytes::Bytes,
    service::TargetSession,
    service::{DialProtocol, ProtocolMeta},
    ProtocolId, SessionId,
};

use crate::error::{ErrorKind, NetworkError};

pub trait NetworkProtocol {
    // TODO: change to TargetProtocol after tentacle 0.3
    fn target() -> DialProtocol;

    fn metas(self) -> Vec<ProtocolMeta>;

    fn message_proto_id() -> ProtocolId;
}

pub trait MessageSender {
    fn send(&self, tar: TargetSession, msg: Bytes, pri: Priority) -> Result<(), NetworkError>;
}

pub trait Compression {
    fn compress(&self, bytes: Bytes) -> Result<Bytes, NetworkError>;
    fn decompress(&self, bytes: Bytes) -> Result<Bytes, NetworkError>;
}

pub trait NetworkContext: Sized {
    fn session_id(&self) -> Result<SessionId, NetworkError>;
    fn set_session_id(&mut self, sid: SessionId) -> Self;
    fn rpc_id(&self) -> Result<u64, NetworkError>;
    fn set_rpc_id(&mut self, rid: u64) -> Self;
}

#[derive(Debug, Clone)]
struct CtxSessionId(SessionId);

impl Cloneable for CtxSessionId {}

#[derive(Debug, Clone)]
struct CtxRpcId(u64);

impl Cloneable for CtxRpcId {}

impl NetworkContext for Context {
    fn session_id(&self) -> Result<SessionId, NetworkError> {
        self.get::<CtxSessionId>("session_id")
            .map(|ctx_sid| ctx_sid.0)
            .ok_or_else(|| ErrorKind::NoSessionId.into())
    }

    fn set_session_id(&mut self, sid: SessionId) -> Self {
        self.with_value::<CtxSessionId>("session_id", CtxSessionId(sid))
    }

    fn rpc_id(&self) -> Result<u64, NetworkError> {
        self.get::<CtxRpcId>("rpc_id")
            .map(|ctx_rid| ctx_rid.0)
            .ok_or_else(|| ErrorKind::NoRpcId.into())
    }

    fn set_rpc_id(&mut self, rid: u64) -> Self {
        self.with_value::<CtxRpcId>("rpc_id", CtxRpcId(rid))
    }
}
