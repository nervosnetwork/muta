use prost::{EncodeError, Message, Oneof};
use protocol::{Bytes, BytesMut};

#[derive(Clone, Copy, PartialEq, Eq, Oneof)]
pub enum PingPayload {
    #[prost(uint32, tag = "1")]
    Ping(u32),
    #[prost(uint32, tag = "2")]
    Pong(u32),
}

#[derive(Clone, PartialEq, Message)]
pub struct PingMessage {
    #[prost(oneof = "PingPayload", tags = "1, 2")]
    pub payload: Option<PingPayload>,
}

impl PingMessage {
    pub fn new_pong(nonce: u32) -> Self {
        PingMessage {
            payload: Some(PingPayload::Pong(nonce)),
        }
    }

    pub fn new_ping(nonce: u32) -> Self {
        PingMessage {
            payload: Some(PingPayload::Ping(nonce)),
        }
    }

    pub fn to_bytes(self) -> Result<Bytes, EncodeError> {
        let mut buf = BytesMut::with_capacity(self.encoded_len());
        self.encode(&mut buf)?;

        Ok(buf.freeze())
    }
}
