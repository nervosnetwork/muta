use std::io;

use common_channel::TrySendError;
use core_network_message::Error as NetMessageError;
use core_runtime::{ConsensusError, StorageError, SynchronizerError, TransactionPoolError};
use core_serialization::CodecError as SerCodecError;

use crate::p2p::{ConnPoolError, ConnectionError};

#[derive(Debug)]
pub enum Error {
    InboundDisconnected,
    InvalidPrivateKey,
    IoError(io::Error),
    ConnectionError(ConnectionError),
    ConnPoolError(ConnPoolError),
    MsgCodecError(NetMessageError),
    UnknownMethod(u32),
    TransactionPoolError(TransactionPoolError),
    SerCodecError(SerCodecError),
    SessionIdNotFound,
    CallbackItemNotFound(u64),
    CallbackItemWrongType(u64),
    ChannelTrySendError(String),
    ConsensusError(ConsensusError),
    StorageError(StorageError),
    SynchronizerError(SynchronizerError),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<ConnectionError> for Error {
    fn from(err: ConnectionError) -> Self {
        Error::ConnectionError(err)
    }
}

impl From<ConnPoolError> for Error {
    fn from(err: ConnPoolError) -> Self {
        Error::ConnPoolError(err)
    }
}

impl From<NetMessageError> for Error {
    fn from(err: NetMessageError) -> Self {
        match err {
            NetMessageError::DecodeError(_) | NetMessageError::EncodeError(_) => {
                Error::MsgCodecError(err)
            }
            NetMessageError::UnknownMethod(method) => Error::UnknownMethod(method),
        }
    }
}

impl From<TransactionPoolError> for Error {
    fn from(err: TransactionPoolError) -> Self {
        Error::TransactionPoolError(err)
    }
}

impl From<SerCodecError> for Error {
    fn from(err: SerCodecError) -> Self {
        Error::SerCodecError(err)
    }
}

impl From<ConsensusError> for Error {
    fn from(err: ConsensusError) -> Self {
        Error::ConsensusError(err)
    }
}

impl From<StorageError> for Error {
    fn from(err: StorageError) -> Self {
        Error::StorageError(err)
    }
}

impl From<SynchronizerError> for Error {
    fn from(err: SynchronizerError) -> Self {
        Error::SynchronizerError(err)
    }
}

impl<T> From<TrySendError<T>> for Error {
    fn from(err: TrySendError<T>) -> Self {
        Error::ChannelTrySendError(format!("cause: {:?}", err))
    }
}
