#![feature(vec_remove_item)]

#[cfg(test)]
mod tests;

mod request_context;
mod sdk;
mod state;
mod store;

use derive_more::{Display, From};

use protocol::{ProtocolError, ProtocolErrorKind};

#[derive(Debug, Display, From)]
pub enum ServiceError {
    #[display(fmt = "method {:?} was not found", _0)]
    NotFoundMethod(String),

    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),
}
impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}
