#[macro_use]
extern crate uint;

pub mod codec;
pub mod traits;
pub mod types;

use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum ProtocolErrorKind {
    // traits
    API,
    Bank,
    Consensus,
    Executor,
    Mempool,
    Network,
    Storage,

    // codec
    Codec,

    // types
    Types,
}

// refer to https://github.com/rust-lang/rust/blob/a17951c4f80eb5208030f91fdb4ae93919fa6b12/src/libstd/io/error.rs#L73
#[derive(Debug)]
pub struct ProtocolError {
    kind:  ProtocolErrorKind,
    error: Box<dyn Error + Send>,
}

impl ProtocolError {
    pub fn new(kind: ProtocolErrorKind, error: Box<dyn Error + Send>) -> Self {
        Self { kind, error }
    }
}

impl Error for ProtocolError {}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[ProtocolError] Kind: {:?} Error: {:?}",
            self.kind, self.error
        )
    }
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;
