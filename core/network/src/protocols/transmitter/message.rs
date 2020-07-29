use bytes::{Buf, BufMut};
use protocol::traits::Priority;
use protocol::{Bytes, BytesMut};
use tentacle::secio::PeerId;
use tentacle::service::TargetSession;
use tentacle::SessionId;

pub enum Recipient {
    Session(TargetSession),
    PeerId(Vec<PeerId>),
}

pub struct TransmitterMessage {
    pub recipient: Recipient,
    pub priority:  Priority,
    pub data:      Bytes,
}

pub struct ReceivedMessage {
    pub session_id: SessionId,
    pub peer_id:    PeerId,
    pub data:       Bytes,
}

pub(crate) struct InternalMessage {
    pub seq:  u64,
    pub eof:  bool,
    pub data: Bytes,
}

impl InternalMessage {
    pub fn encode(self) -> Bytes {
        let eof = if self.eof { 1u8 } else { 0u8 };

        let mut buf = BytesMut::new();
        buf.put_u64(self.seq);
        buf.put_u8(eof);
        buf.extend_from_slice(self.data.as_ref());

        buf.freeze()
    }

    pub fn decode(mut bytes: Bytes) -> Self {
        let seq = bytes.get_u64();
        let eof = bytes.get_u8() == 1;

        InternalMessage {
            seq,
            eof,
            data: bytes,
        }
    }
}
