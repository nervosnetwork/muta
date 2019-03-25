use std::error::Error;
use std::fmt;

use bytes::{BytesMut, IntoBuf};
use futures::future::{result, Future};
use prost::{DecodeError, EncodeError, Message};

macro_rules! generate_module_for {
    ([$( $name:ident, )+]) => {
        $( generate_module_for!($name); )+
    };
    ([$( $name:ident ),+]) => {
        $( generate_module_for!($name); )+
    };
    ($name:ident) => {
        pub mod $name {
            include!(concat!(env!("OUT_DIR"), "/", stringify!($name), ".rs"));
        }
    };
}

generate_module_for!([block, transaction, receipt,]);

#[derive(Default)]
pub struct AsyncCodec;

impl AsyncCodec {
    pub fn decode<T: 'static + Message + Default>(
        data: Vec<u8>,
    ) -> Box<Future<Item = T, Error = CodecError> + Send> {
        Box::new(result(SyncCodec::decode(data)))
    }

    pub fn encode<T: Message>(
        msg: &T,
        buf: &mut BytesMut,
    ) -> Box<Future<Item = (), Error = CodecError> + Send> {
        Box::new(result(SyncCodec::encode(msg, buf)))
    }

    pub fn encoded_len<T: Message>(msg: &T) -> usize {
        SyncCodec::encoded_len(msg)
    }
}

#[derive(Default)]
pub struct SyncCodec;

impl SyncCodec {
    pub fn decode<T: 'static + Message + Default>(data: Vec<u8>) -> Result<T, CodecError> {
        T::decode(data.into_buf()).map_err(CodecError::Decode)
    }

    pub fn encode<T: Message>(msg: &T, buf: &mut BytesMut) -> Result<(), CodecError> {
        msg.encode(buf).map_err(CodecError::Encode)?;
        Ok(())
    }

    pub fn encoded_len<T: Message>(msg: &T) -> usize {
        msg.encoded_len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Decode(DecodeError),
    Encode(EncodeError),
}

impl Error for CodecError {
    fn description(&self) -> &str {
        match *self {
            CodecError::Decode(_) => "serialization decode error",
            CodecError::Encode(_) => "serialization encode error",
        }
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CodecError::Decode(ref err) => format!("serialization decode error: {:?}", err),
            CodecError::Encode(ref err) => format!("serialization encode error: {:?}", err),
        };
        write!(f, "{}", printable)
    }
}

impl From<DecodeError> for CodecError {
    fn from(err: DecodeError) -> Self {
        CodecError::Decode(err)
    }
}

impl From<EncodeError> for CodecError {
    fn from(err: EncodeError) -> Self {
        CodecError::Encode(err)
    }
}
