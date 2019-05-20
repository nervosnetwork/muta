use std::io;

use tentacle::error::Error as TentacleError;
use tentacle::service::ServiceError as TentacleServiceError;

use core_network_message::Error as NetMessageError;
use core_runtime::{ConsensusError, StorageError, SynchronizerError, TransactionPoolError};
use core_serialization::CodecError as SerCodecError;

#[derive(Debug)]
pub enum Error {
    InboundDisconnected,
    InvalidPrivateKey,
    IoError(io::Error),
    ConnectionError(TentacleError),
    ConnPollError(TentacleServiceError),
    MsgCodecError(NetMessageError),
    UnknownMethod(u32),
    TransactionPoolError(TransactionPoolError),
    SerCodecError(SerCodecError),
    SessionIdNotFound,
    CallbackItemNotFound(u64),
    CallbackTrySendError,
    ConsensusError(ConsensusError),
    StorageError(StorageError),
    SynchronizerError(SynchronizerError),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<TentacleError> for Error {
    fn from(err: TentacleError) -> Self {
        Error::ConnectionError(err)
    }
}

impl From<TentacleServiceError> for Error {
    fn from(err: TentacleServiceError) -> Self {
        Error::ConnPollError(err)
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
