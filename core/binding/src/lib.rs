#[cfg(test)]
mod tests;

mod state;
mod store;

use std::error::Error;

use derive_more::{Display, From};

use protocol::{ProtocolError, ProtocolErrorKind};

use crate::store::StoreType;

#[derive(Debug, Display, From)]
pub enum BindingError {
    Store(StoreType),
}

impl Error for BindingError {}

impl From<BindingError> for ProtocolError {
    fn from(err: BindingError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}
