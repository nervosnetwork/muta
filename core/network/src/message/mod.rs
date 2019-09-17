pub mod serde;

use derive_more::Constructor;
use prost::Message;
use tentacle::{bytes::Bytes, SessionId};

use crate::{
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
};

#[derive(Constructor)]
pub struct RawSessionMessage {
    pub(crate) sid: SessionId,
    pub(crate) msg: Bytes,
}

#[derive(Message)]
pub struct NetworkMessage {
    #[prost(string, tag = "1")]
    pub url: String,

    #[prost(bytes, tag = "2")]
    pub content: Vec<u8>,
}

impl NetworkMessage {
    pub fn new(endpoint: Endpoint, content: Bytes) -> Self {
        NetworkMessage {
            url:     endpoint.full_url().to_owned(),
            content: content.to_vec(),
        }
    }

    pub async fn encode(self) -> Result<Bytes, NetworkError> {
        let mut buf = Vec::with_capacity(self.encoded_len());

        <Self as Message>::encode(&self, &mut buf)
            .map_err(|e| ErrorKind::BadMessage(Box::new(e)))?;

        Ok(Bytes::from(buf))
    }

    pub async fn decode(bytes: Bytes) -> Result<Self, NetworkError> {
        <Self as Message>::decode(bytes).map_err(|e| ErrorKind::BadMessage(Box::new(e)).into())
    }
}

#[derive(Constructor)]
pub struct SessionMessage {
    pub(crate) sid: SessionId,
    pub(crate) msg: NetworkMessage,
}
