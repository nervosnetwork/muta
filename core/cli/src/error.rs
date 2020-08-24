use std::error::Error;

use derive_more::{Display, From};

use protocol::{ProtocolError, ProtocolErrorKind};

#[derive(Debug, Display, From)]
pub enum CliError {
    #[display(fmt = "input is not a valid JSON format for target, {:?}", _0)]
    JSONFormat(serde_json::error::Error),

    #[display(fmt = "grammar error")]
    Grammar,

    #[display(fmt = "path not found: {}", _0)]
    Path(String),

    #[display(fmt = "io operation fails: {:?}", _0)]
    IO(std::io::Error),

    #[display(fmt = "io operation fails: {:?}", _0)]
    IO2(fs_extra::error::Error),

    #[display(fmt = "block for height {} not found", _0)]
    BlockNotFound(u64),

    #[display(fmt = "parsing error")]
    Parse,

    #[display(fmt = "unsupported command")]
    UnsupportedCommand,
}

impl Error for CliError {}

impl From<CliError> for ProtocolError {
    fn from(err: CliError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
    }
}
