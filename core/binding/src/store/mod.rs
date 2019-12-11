mod array;
mod map;
mod primitive;

use derive_more::{Display, From};

pub use primitive::{DefaultStoreBool, DefaultStoreString, DefaultStoreUint64};

#[derive(Debug, Display, From)]
pub enum StoreType {
    // The key not existed
    GetNone,

    DecodeError,

    // Overflow when calculating
    Overflow,
}
