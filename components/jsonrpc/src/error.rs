use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum RpcError {
    // Str(String),
    IO(io::Error),
}

impl error::Error for RpcError {}
impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // RpcError::Str(s) => return write!(f, "{}", s),
            RpcError::IO(e) => return write!(f, "{}", e),
        };
    }
}

impl From<io::Error> for RpcError {
    fn from(error: io::Error) -> Self {
        RpcError::IO(error)
    }
}
