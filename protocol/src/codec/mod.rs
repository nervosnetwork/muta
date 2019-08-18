// TODO: change Vec<u8> to Bytes
// pin: https://github.com/danburkert/prost/pull/190

#[macro_use]
mod r#macro;
pub mod primitive;
pub mod transaction;

use std::error::Error;

use async_trait::async_trait;
use bytes::Bytes;
use derive_more::{Display, From};

use crate::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[async_trait]
pub trait ProtocolCodec: Sized + Send + ProtocolCodecSync {
    // Note: We take mut reference so that it can be pinned. This removes Sync
    // requirement.
    async fn encode(&mut self) -> ProtocolResult<Bytes>;

    async fn decode<B: Into<Bytes> + Send>(bytes: B) -> ProtocolResult<Self>;
}

// Sync version is still useful in some cases, for example, use in Stream.
// This also work around #[async_trait] problem inside macro
#[doc(hidden)]
pub trait ProtocolCodecSync: Sized + Send {
    fn encode_sync(&self) -> ProtocolResult<Bytes>;

    fn decode_sync(bytes: Bytes) -> ProtocolResult<Self>;
}

#[async_trait]
impl<T: ProtocolCodecSync + 'static> ProtocolCodec for T {
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        <T as ProtocolCodecSync>::encode_sync(self)
    }

    async fn decode<B: Into<Bytes> + Send>(bytes: B) -> ProtocolResult<Self> {
        let bytes: Bytes = bytes.into();

        <T as ProtocolCodecSync>::decode_sync(bytes)
    }
}

#[derive(Debug, From, Display)]
pub enum CodecError {
    #[display(fmt = "prost encode: {}", _0)]
    ProtobufEncode(prost::EncodeError),

    #[display(fmt = "prost decode: {}", _0)]
    ProtobufDecode(prost::DecodeError),

    #[display(fmt = "{} missing field {}", r#type, field)]
    MissingField {
        r#type: &'static str,
        field:  &'static str,
    },

    #[display(fmt = "invalid contract type {}", _0)]
    InvalidContractType(i32),

    #[display(fmt = "wrong bytes length: {{ expect: {}, got: {} }}", expect, real)]
    WrongBytesLength { expect: usize, real: usize },
}

impl Error for CodecError {}

// TODO: derive macro
impl From<CodecError> for ProtocolError {
    fn from(err: CodecError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Codec, Box::new(err))
    }
}
