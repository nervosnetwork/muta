#![feature(test)]
#![allow(clippy::mutable_key_type)]

pub mod codec;
pub mod fixed_codec;
pub mod traits;
pub mod types;

use std::error::Error;

pub use async_trait::async_trait;
pub use bytes::{Buf, BufMut, Bytes, BytesMut};
use derive_more::{Constructor, Display};

#[derive(Debug, Clone)]
pub enum ProtocolErrorKind {
    // traits
    API,
    Consensus,
    Executor,
    Mempool,
    Network,
    Storage,
    Runtime,
    Binding,
    BindingMacro,
    Service,
    Main,

    // codec
    Codec,

    // fixed codec
    FixedCodec,

    // types
    Types,

    // metric
    Metric,
}

// refer to https://github.com/rust-lang/rust/blob/a17951c4f80eb5208030f91fdb4ae93919fa6b12/src/libstd/io/error.rs#L73
#[derive(Debug, Constructor, Display)]
#[display(fmt = "[ProtocolError] Kind: {:?} Error: {:?}", kind, error)]
pub struct ProtocolError {
    kind:  ProtocolErrorKind,
    error: Box<dyn Error + Send>,
}

impl From<ProtocolError> for Box<dyn Error + Send> {
    fn from(error: ProtocolError) -> Self {
        Box::new(error) as Box<dyn Error + Send>
    }
}

impl Error for ProtocolError {}

pub type ProtocolResult<T> = Result<T, ProtocolError>;
