use derive_more::Display;
use protocol::{ProtocolError, ProtocolErrorKind};
use std::any::Any;

#[derive(Debug, Display)]
pub enum ExecutorError {
    #[display(fmt = "service {:?} was not found", service)]
    NotFoundService { service: String },
    #[display(fmt = "service {:?} method {:?} was not found", service, method)]
    NotFoundMethod { service: String, method: String },
    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),

    #[display(fmt = "Init service genesis failed: {:?}", _0)]
    InitService(String),
    #[display(fmt = "Query service failed: {:?}", _0)]
    QueryService(String),
    #[display(fmt = "Call service failed: {:?}", _0)]
    CallService(String),

    #[display(fmt = "Tx hook panic: {:?}", _0)]
    TxHook(Box<dyn Any + Send>),
}

impl std::error::Error for ExecutorError {}

impl From<ExecutorError> for ProtocolError {
    fn from(err: ExecutorError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
