use std::default::Default;
use std::fmt::Debug;

use bytes::Bytes;

pub mod common;
pub mod consensus;
pub mod method;
pub mod sync;
pub mod tx_pool;

pub use method::Method;

// TODO: try flatbuffer or cap'n proto, or maybe serde to binary
// TODO: change method to u8 but protobuf doesn't support it
#[derive(Clone, PartialEq, prost::Message)]
pub struct Message {
    #[prost(uint32, tag = "1")]
    pub method: u32,
    #[prost(uint64, tag = "2")]
    pub data_size: u64,
    #[prost(bytes, tag = "3")]
    pub data: Vec<u8>,
}

pub trait Codec: Sized {
    type Error: Debug;

    fn encode(&self) -> Result<Bytes, Self::Error>;
    fn decode(raw: &[u8]) -> Result<Self, Self::Error>;
}

#[derive(Debug)]
pub enum Error {
    EncodeError(prost::EncodeError),
    DecodeError(prost::DecodeError),
    UnknownMethod(u32),
}

impl<T> Codec for T
where
    T: prost::Message + Default,
{
    type Error = Error;

    fn encode(&self) -> Result<Bytes, Self::Error> {
        let mut bytes = vec![];

        <T as prost::Message>::encode(self, &mut bytes).map_err(Error::EncodeError)?;

        Ok(Bytes::from(bytes))
    }

    fn decode(raw: &[u8]) -> Result<T, Self::Error> {
        <T as prost::Message>::decode(raw.to_owned()).map_err(Error::DecodeError)
    }
}
