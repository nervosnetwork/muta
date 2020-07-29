use std::{error::Error, num::ParseIntError};

use derive_more::Display;
use tentacle::{
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
    ProtocolId, SessionId,
};

use protocol::{types::Address, Bytes, ProtocolError, ProtocolErrorKind};

use crate::common::ConnectedAddr;

#[derive(Debug, Display)]
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

    #[display(fmt = "kind: remote peer id not found in context")]
    NoRemotePeerId,

    #[display(fmt = "kind: rpc id not found in context")]
    NoRpcId,

    #[display(fmt = "kind: rpc future dropped {:?}", _0)]
    RpcDropped(Option<ConnectedAddr>),

    #[display(fmt = "kind: rpc timeout {:?}", _0)]
    RpcTimeout(Option<ConnectedAddr>),

    #[display(fmt = "kind: not reactor register for {}", _0)]
    NoReactor(String),

    #[display(
        fmt = "kind: cannot create chain address from bytes {:?} {}",
        pubkey,
        cause
    )]
    NoChainAddress {
        pubkey: Bytes,
        cause:  Box<dyn Error + Send>,
    },

    #[display(fmt = "kind: public key {:?} not match {:?}", pubkey, id)]
    PublicKeyNotMatchId { pubkey: PublicKey, id: PeerId },

    #[display(fmt = "kind: untaggable {}", _0)]
    Untaggable(String),

    #[display(fmt = "kind: internal {}", _0)]
    Internal(String),
}

impl Error for ErrorKind {}

#[derive(Debug, Display)]
#[display(fmt = "peer id not found in {}", _0)]
pub struct PeerIdNotFound(pub(crate) Multiaddr);

impl Error for PeerIdNotFound {}

#[derive(Debug, Display)]
pub enum NetworkError {
    #[display(fmt = "io error: {}", _0)]
    IoError(std::io::Error),

    #[display(fmt = "temporary unavailable, try again later")]
    Busy,

    #[display(fmt = "send incompletely, blocked {:?}, other {:?}", blocked, other)]
    Send {
        blocked: Option<Vec<SessionId>>,
        other:   Option<Box<dyn Error + Send>>,
    },

    #[display(
        fmt = "send incompletely, unconnected {:?}, other {:?}",
        unconnected,
        other
    )]
    MultiCast {
        unconnected: Option<Vec<PeerId>>,
        other:       Option<Box<dyn Error + Send>>,
    },

    #[display(fmt = "shutdown")]
    Shutdown,

    #[display(fmt = "unexected error: {}", _0)]
    UnexpectedError(Box<dyn Error + Send>),

    #[display(fmt = "cannot decode public key bytes")]
    InvalidPublicKey,

    #[display(fmt = "cannot decode private key bytes")]
    InvalidPrivateKey,

    #[display(fmt = "cannot decode peer id")]
    InvalidPeerId,

    #[display(fmt = "unsupported peer address {}", _0)]
    UnexpectedPeerAddr(String),

    #[display(fmt = "unknown endpoint scheme {}", _0)]
    UnexpectedScheme(String),

    #[display(fmt = "cannot serde encode or decode: {}", _0)]
    SerdeError(Box<dyn Error + Send>),

    #[display(fmt = "malformat or exceed maximum length, /[scheme]/[name]/[method] etc")]
    NotEndpoint,

    #[display(fmt = "{:?} account addrs aren't connecting, try connect them", miss)]
    PartialRouteMessage { miss: Vec<Address> },

    #[display(fmt = "remote response {}", _0)]
    RemoteResponse(Box<dyn Error + Send>),

    #[display(fmt = "trust max history should be longer than {} secs", _0)]
    SmallTrustMaxHistory(u64),

    #[display(fmt = "transport {}", _0)]
    Transport(tentacle::error::TransportErrorKind),

    #[display(fmt = "internal error: {}", _0)]
    Internal(Box<dyn Error + Send>),
}

impl Error for NetworkError {}

impl From<PeerIdNotFound> for NetworkError {
    fn from(err: PeerIdNotFound) -> NetworkError {
        NetworkError::Internal(Box::new(err))
    }
}

impl From<ErrorKind> for NetworkError {
    fn from(kind: ErrorKind) -> NetworkError {
        NetworkError::Internal(Box::new(kind))
    }
}

impl From<Box<bincode::ErrorKind>> for NetworkError {
    fn from(kind: Box<bincode::ErrorKind>) -> NetworkError {
        NetworkError::SerdeError(Box::new(kind))
    }
}

impl From<NetworkError> for ProtocolError {
    fn from(err: NetworkError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Network, Box::new(err))
    }
}

impl From<std::io::Error> for NetworkError {
    fn from(err: std::io::Error) -> NetworkError {
        NetworkError::IoError(err)
    }
}

impl From<tentacle::error::TransportErrorKind> for NetworkError {
    fn from(err: tentacle::error::TransportErrorKind) -> NetworkError {
        NetworkError::Transport(err)
    }
}

impl From<NetworkError> for Box<dyn Error + Send> {
    fn from(err: NetworkError) -> Box<dyn Error + Send> {
        err.boxed()
    }
}

impl NetworkError {
    pub fn boxed(self) -> Box<dyn Error + Send> {
        Box::new(self) as Box<dyn Error + Send>
    }
}
