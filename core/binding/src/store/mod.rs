mod array;
mod map;
mod primitive;

use derive_more::{Display, From};

use protocol::{ProtocolError, ProtocolErrorKind};

pub use primitive::{DefaultStoreBool, DefaultStoreString, DefaultStoreUint64};

#[derive(Debug, Display, From)]
pub enum StoreError {
    #[display(fmt = "the key not existed")]
    GetNone,

    #[display(fmt = "decode error")]
    DecodeError,

    #[display(fmt = "overflow when calculating")]
    Overflow,
}

impl std::error::Error for StoreError {}

impl From<StoreError> for ProtocolError {
    fn from(err: StoreError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}
