use bytes::Bytes;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub trait ChainInterface {
    fn get_storage(&self, key: &Bytes) -> ProtocolResult<Bytes>;

    fn set_storage(&mut self, key: Bytes, val: Bytes) -> ProtocolResult<()>;
}
