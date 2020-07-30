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

pub(crate) struct SeqChunkMessage {
    pub seq:  u64,
    pub eof:  bool,
    pub data: Bytes,
}

impl SeqChunkMessage {
    pub fn encode(self) -> Bytes {
        let eof = if self.eof { 1u8 } else { 0u8 };
        let mut buf = BytesMut::with_capacity(9 + self.data.len());

        buf.put_u64(self.seq);
        buf.put_u8(eof);
        buf.extend_from_slice(self.data.as_ref());

        buf.freeze()
    }

    // Note: already check data size in protocol received.
    pub fn decode(mut bytes: Bytes) -> Self {
        let data = bytes.split_off(9);
        let seq = bytes.get_u64();
        let eof = bytes.get_u8() == 1;

        SeqChunkMessage { seq, eof, data }
    }
}

#[cfg(test)]
mod tests {
    use super::SeqChunkMessage;

    use protocol::Bytes;

    #[test]
    fn test_internal_message_codec() {
        let data = b"hello muta";

        let chunk = SeqChunkMessage {
            seq:  1u64,
            eof:  false,
            data: Bytes::from_static(data),
        };

        let chunk = SeqChunkMessage::decode(chunk.encode());
        assert_eq!(chunk.data, Bytes::from_static(data));
        assert_eq!(chunk.eof, false);
    }
}
