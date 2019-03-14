use super::RawMessage;

use bytes::Bytes;

/// `Message` codec
pub trait Codec: Sized {
    /// Encode `Message` type to transport data type
    fn encode(self) -> RawMessage;

    /// Decode raw bytes to `Message` type
    fn decode(raw: &[u8]) -> Result<Self, ()>;
}

// Default implement for `RawMessage`
#[cfg(not(feature = "prost-message"))]
impl Codec for RawMessage {
    fn encode(self) -> RawMessage {
        self
    }

    fn decode(raw: &[u8]) -> Result<RawMessage, ()> {
        Ok(Bytes::from(raw))
    }
}

// Implement `prost` out-of-box support
#[cfg(feature = "prost-message")]
impl<TMessage: prost::Message + std::default::Default> Codec for TMessage {
    fn encode(self) -> RawMessage {
        let mut msg = vec![];

        if let Err(err) = <TMessage as prost::Message>::encode(&self, &mut msg) {
            // system should not provide non-encodeable message
            // this means fatal error, but dont panic.
            log::error!("protocol [transmission]: *! encode failure: {:?}", err);
        }

        Bytes::from(msg)
    }

    fn decode(raw: &[u8]) -> Result<TMessage, ()> {
        <TMessage as prost::Message>::decode(raw.to_owned())
            .map_err(|err| log::error!("protocol [transmission]: *! decode failure: {:?}", err))
    }
}
