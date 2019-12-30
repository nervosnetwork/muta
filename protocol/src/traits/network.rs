use std::{error::Error, fmt::Debug};

use async_trait::async_trait;
use bytes::Bytes;
use derive_more::Display;
use serde::{Deserialize, Serialize};

use crate::{
    traits::Context, types::UserAddress, ProtocolError, ProtocolErrorKind, ProtocolResult,
};

#[derive(Debug)]
pub enum Priority {
    High,
    Normal,
}

#[async_trait]
pub trait MessageCodec: Sized + Send + Debug + 'static {
    async fn encode(&mut self) -> ProtocolResult<Bytes>;

    async fn decode(bytes: Bytes) -> ProtocolResult<Self>;
}

#[derive(Debug, Display)]
#[display(fmt = "cannot serde encode or decode: {}", _0)]
struct SerdeError(Box<dyn Error + Send>);

impl Error for SerdeError {}

impl From<SerdeError> for ProtocolError {
    fn from(err: SerdeError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Network, Box::new(err))
    }
}

#[async_trait]
impl<T> MessageCodec for T
where
    T: Serialize + for<'a> Deserialize<'a> + Send + Debug + 'static,
{
    async fn encode(&mut self) -> ProtocolResult<Bytes> {
        let bytes = bincode::serialize(self).map_err(|e| SerdeError(Box::new(e)))?;

        Ok(bytes.into())
    }

    async fn decode(bytes: Bytes) -> ProtocolResult<Self> {
        bincode::deserialize::<T>(&bytes.as_ref()).map_err(|e| SerdeError(Box::new(e)).into())
    }
}

#[async_trait]
pub trait Gossip: Send + Sync {
    async fn broadcast<M>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<()>
    where
        M: MessageCodec;

    async fn users_cast<M>(
        &self,
        cx: Context,
        end: &str,
        users: Vec<UserAddress>,
        msg: M,
        p: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec;
}

#[async_trait]
pub trait Rpc: Send + Sync {
    async fn call<M, R>(&self, ctx: Context, end: &str, msg: M, pri: Priority) -> ProtocolResult<R>
    where
        M: MessageCodec,
        R: MessageCodec;

    async fn response<M>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<()>
    where
        M: MessageCodec;
}

#[async_trait]
pub trait MessageHandler: Sync + Send + 'static {
    type Message: MessageCodec;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()>;
}
