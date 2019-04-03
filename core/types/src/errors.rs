use std::error;
use std::fmt;

use numext_fixed_hash_core::FixedHashError;

#[derive(Debug)]
pub enum CoreTypesError {
    ParseHexError(FixedHashError),
}

impl error::Error for CoreTypesError {}
impl fmt::Display for CoreTypesError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CoreTypesError::ParseHexError(ref err) => format!("parse hex error: {:?}", err),
        };
        write!(f, "{}", printable)
    }
}

impl From<FixedHashError> for CoreTypesError {
    fn from(err: FixedHashError) -> Self {
        CoreTypesError::ParseHexError(err)
    }
}
