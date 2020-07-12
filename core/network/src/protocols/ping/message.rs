use super::message_mol;

use molecule::prelude::{Builder, Entity, Reader};
use protocol::Bytes;

pub enum PingPayload {
    Ping(u32),
    Pong(u32),
}

pub struct PingMessage;

impl PingMessage {
    pub fn build_ping(nonce: u32) -> Bytes {
        let nonce_le = nonce.to_le_bytes();
        let nonce = message_mol::Uint32::new_builder()
            .nth0(nonce_le[0].into())
            .nth1(nonce_le[1].into())
            .nth2(nonce_le[2].into())
            .nth3(nonce_le[3].into())
            .build();
        let ping = message_mol::Ping::new_builder().nonce(nonce).build();
        let payload = message_mol::PingPayload::new_builder().set(ping).build();

        message_mol::PingMessage::new_builder()
            .payload(payload)
            .build()
            .as_bytes()
    }

    pub fn build_pong(nonce: u32) -> Bytes {
        let nonce_le = nonce.to_le_bytes();
        let nonce = message_mol::Uint32::new_builder()
            .nth0(nonce_le[0].into())
            .nth1(nonce_le[1].into())
            .nth2(nonce_le[2].into())
            .nth3(nonce_le[3].into())
            .build();
        let pong = message_mol::Pong::new_builder().nonce(nonce).build();
        let payload = message_mol::PingPayload::new_builder().set(pong).build();

        message_mol::PingMessage::new_builder()
            .payload(payload)
            .build()
            .as_bytes()
    }

    #[allow(clippy::cast_ptr_alignment)]
    pub fn decode(data: &[u8]) -> Option<PingPayload> {
        let reader = message_mol::PingMessageReader::from_compatible_slice(data).ok()?;
        match reader.payload().to_enum() {
            message_mol::PingPayloadUnionReader::Ping(reader) => {
                let le = reader.nonce().raw_data().as_ptr() as *const u32;
                Some(PingPayload::Ping(u32::from_le(unsafe { *le })))
            }
            message_mol::PingPayloadUnionReader::Pong(reader) => {
                let le = reader.nonce().raw_data().as_ptr() as *const u32;
                Some(PingPayload::Pong(u32::from_le(unsafe { *le })))
            }
        }
    }
}
