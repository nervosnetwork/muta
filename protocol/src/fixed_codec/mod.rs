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
pub trait FixedCodec: Sized {
    fn encode_fixed(&self) -> ProtocolResult<Bytes>;

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self>;
}

#[derive(Debug, Display, From)]
pub enum FixedCodecError {
    Decoder(rlp::DecoderError),

    StringUTF8(std::string::FromUtf8Error),

    #[display(fmt = "wrong bytes of bool")]
    DecodeBool,

    #[display(fmt = "wrong bytes of u8")]
    DecodeUint8,
}

impl Error for FixedCodecError {}

impl From<FixedCodecError> for ProtocolError {
    fn from(err: FixedCodecError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::FixedCodec, Box::new(err))
    }
}
