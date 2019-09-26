use std::error::Error;

use derive_more::{Display, From};

use protocol::{ProtocolError, ProtocolErrorKind};

#[derive(Debug, Display, From)]
pub enum DexError {
    /// error throw from contract
    #[display(fmt = "dex contract err: {}", _0)]
    Contract(String),

    /// error from adapter, when accessing data
    #[display(fmt = "dex adapter err: {}", _0)]
    Adapter(String),

    #[display(fmt = "dex types err: {}", _0)]
    FixedTypesError(rlp::DecoderError),

    #[display(fmt = "dex method args err: {}", _0)]
    ArgsError(String),
}

impl Error for DexError {}

impl From<DexError> for ProtocolError {
    fn from(err: DexError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
