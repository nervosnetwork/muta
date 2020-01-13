use std::error;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    VM(ckb_vm::Error),
    ExitCodeError,
}

impl error::Error for Error {}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::VM(e) => return write!(f, "{:?}", e),
            Error::ExitCodeError => return write!(f, "ExitCodeError"),
        };
    }
}

impl From<ckb_vm::Error> for Error {
    fn from(error: ckb_vm::Error) -> Self {
        Error::VM(error)
    }
}
