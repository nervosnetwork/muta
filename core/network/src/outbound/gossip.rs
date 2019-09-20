use async_trait::async_trait;
use protocol::{
    traits::{Context, Gossip, MessageCodec, Priority},
    types::UserAddress,
    ProtocolResult,
};
use tentacle::{bytes::Bytes, service::TargetSession};

use crate::{
    endpoint::Endpoint,
    error::NetworkError,
    message::NetworkMessage,
    traits::{Compression, MessageSender},
};

#[derive(Clone)]
pub struct NetworkGossip<S, C> {
    sender:      S,
    compression: C,
}

impl<S, C> NetworkGossip<S, C>
where
    S: MessageSender + Sync + Send + Clone,
    C: Compression + Sync + Send + Clone,
{
    pub fn new(sender: S, compression: C) -> Self {
        NetworkGossip {
            sender,
            compression,
        }
    }

    async fn package_message<M>(
        &self,
        _ctx: Context,
        end: &str,
        mut msg: M,
    ) -> ProtocolResult<Bytes>
    where
        M: MessageCodec,
    {
        let endpoint = end.parse::<Endpoint>()?;
        let data = msg.encode().await?;
        let net_msg = NetworkMessage::new(endpoint, data).encode().await?;
        let msg = self.compression.compress(net_msg)?;

        Ok(msg)
    }

    fn send(
        &self,
        _ctx: Context,
        tar: TargetSession,
        msg: Bytes,
        pri: Priority,
    ) -> Result<(), NetworkError> {
        self.sender.send(tar, msg, pri)
    }

    async fn users_send(
        &self,
        _ctx: Context,
        users: Vec<UserAddress>,
        msg: Bytes,
        pri: Priority,
    ) -> Result<(), NetworkError> {
        self.sender.users_send(users, msg, pri).await
    }
}

#[async_trait]
impl<S, C> Gossip for NetworkGossip<S, C>
where
    S: MessageSender + Sync + Send + Clone,
    C: Compression + Sync + Send + Clone,
{
    async fn broadcast<M>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        let msg = self.package_message(cx.clone(), end, msg).await?;
        self.send(cx, TargetSession::All, msg, p)?;

        Ok(())
    }

    async fn users_cast<M>(
        &self,
        cx: Context,
        end: &str,
        users: Vec<UserAddress>,
        msg: M,
        p: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        let msg = self.package_message(cx.clone(), end, msg).await?;
        self.users_send(cx, users, msg, p).await?;

        Ok(())
    }
}
