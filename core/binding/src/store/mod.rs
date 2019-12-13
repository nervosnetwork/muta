mod array;
mod map;
mod primitive;

use derive_more::{Display, From};

use protocol::{ProtocolError, ProtocolErrorKind};

pub use array::DefaultStoreArray;
pub use map::{DefaultStoreMap, FixedKeys};
pub use primitive::{DefaultStoreBool, DefaultStoreString, DefaultStoreUint64};

#[derive(Debug, Display, From)]
pub enum StoreError {
    #[display(fmt = "the key not existed")]
    GetNone,

    #[display(fmt = "access array out of range")]
    OutRange,

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
