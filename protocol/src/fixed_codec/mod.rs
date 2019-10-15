#[macro_use]
mod r#macro;
pub mod epoch;
pub mod genesis;
pub mod primitive;
pub mod receipt;
#[cfg(test)]
pub mod tests;
pub mod transaction;

use std::error::Error;

use bytes::Bytes;
use derive_more::{Display, From};

use crate::{ProtocolError, ProtocolErrorKind, ProtocolResult};

// Consistent serialization trait using rlp-algorithm
pub trait ProtocolFixedCodec: Sized {
    fn encode_fixed(&self) -> ProtocolResult<Bytes>;

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self>;
}

#[derive(Debug, Display, From)]
pub enum FixedCodecError {
    Decoder(rlp::DecoderError),
}

impl Error for FixedCodecError {}

impl From<FixedCodecError> for ProtocolError {
    fn from(err: FixedCodecError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::FixedCodec, Box::new(err))
    }
}

pub fn bytes_to_u64(bytes: &[u8]) -> u64 {
    let mut nonce_bytes = [0u8; 8];
    nonce_bytes.copy_from_slice(bytes);
    u64::from_be_bytes(nonce_bytes)
}
