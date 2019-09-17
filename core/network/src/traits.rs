use protocol::traits::{NContext, Priority};
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

pub trait NetworkContext {
    fn session_id(&self) -> Result<SessionId, NetworkError>;
    fn set_session_id(&mut self, sid: SessionId);
    fn rpc_id(&self) -> Result<u64, NetworkError>;
    fn set_rpc_id(&mut self, rid: u64);
}

impl NetworkContext for NContext {
    fn session_id(&self) -> Result<SessionId, NetworkError> {
        self.get("session_id")
            .map(|ref_sid| SessionId::new(*ref_sid))
            .ok_or_else(|| ErrorKind::NoSessionId.into())
    }

    fn set_session_id(&mut self, sid: SessionId) {
        self.insert("session_id".to_owned(), sid.value());
    }

    fn rpc_id(&self) -> Result<u64, NetworkError> {
        self.get("rpc_id")
            .map(|ref_rid| *ref_rid as u64)
            .ok_or_else(|| ErrorKind::NoRpcId.into())
    }

    fn set_rpc_id(&mut self, rid: u64) {
        // FIXME: truncated rid
        self.insert("rpc_id".to_owned(), rid as usize);
    }
}
