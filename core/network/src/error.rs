use std::{error::Error, num::ParseIntError};

use derive_more::{Display, From};
use tentacle::{ProtocolId, SessionId};

use protocol::{ProtocolError, ProtocolErrorKind};

#[derive(Debug, Display, From)]
pub enum ErrorKind {
    #[display(fmt = "{} offline", _0)]
    Offline(&'static str),

    #[display(fmt = "protocol {} missing", _0)]
    MissingProtocol(&'static str),

    #[display(fmt = "kind: bad protocl logic code")]
    BadProtocolHandle {
        proto_id: ProtocolId,
        cause:    Box<dyn Error + Send>,
    },

    #[display(fmt = "kind: session blocked, may be bad protocol code")]
    SessionBlocked,

    #[display(fmt = "kind: given string isn't an id: {}", _0)]
    NotIdString(ParseIntError),

    #[display(fmt = "kind: unable to encode or decode: {}", _0)]
    BadMessage(Box<dyn Error + Send>),

    #[display(fmt = "kind: unknown rid {} from session {}", rid, sid)]
    UnknownRpc { sid: SessionId, rid: u64 },

    #[display(fmt = "kind: unexpected rpc sender, wrong type")]
    UnexpectedRpcSender,

    #[display(fmt = "kind: more than one arc rpc sender, cannot unwrap it")]
    MoreArcRpcSender,

    #[display(fmt = "kind: session id not found in context")]
    NoSessionId,

    #[display(fmt = "kind: rpc id not found in context")]
    NoRpcId,

    #[display(fmt = "kind: rpc future dropped")]
    RpcDropped,

    #[display(fmt = "kind: not reactor register for {}", _0)]
    NoReactor(String),
}

impl Error for ErrorKind {}

#[derive(Debug, Display, From)]
pub enum NetworkError {
    #[display(fmt = "io error: {}", _0)]
    IoError(std::io::Error),

    #[display(fmt = "temporary unavailable, try again later")]
    Busy,

    #[display(fmt = "shutdown")]
    Shutdown,

    #[display(fmt = "unexected error: {}", _0)]
    UnexpectedError(Box<dyn Error + Send>),

    #[display(fmt = "cannot decode public key bytes")]
    InvalidPublicKey,

    #[display(fmt = "cannot decode private key bytes")]
    InvalidPrivateKey,

    #[display(fmt = "unknown endpoint scheme {}", _0)]
    UnexpectedScheme(String),

    #[display(fmt = "cannot serde encode or decode: {}", _0)]
    SerdeError(Box<dyn Error + Send>),

    #[display(fmt = "malformat or exceed maximum length, /[scheme]/[name]/[method] etc")]
    NotEndpoint,

    #[display(fmt = "internal error: {}", _0)]
    Internal(Box<dyn Error + Send>),
}

impl Error for NetworkError {}

impl From<ErrorKind> for NetworkError {
    fn from(kind: ErrorKind) -> NetworkError {
        NetworkError::Internal(Box::new(kind))
    }
}

impl From<NetworkError> for ProtocolError {
    fn from(err: NetworkError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Network, Box::new(err))
    }
}
