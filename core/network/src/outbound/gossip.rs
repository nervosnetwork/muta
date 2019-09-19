use async_trait::async_trait;
use protocol::{
    traits::{Gossip, MessageCodec, Priority},
    ProtocolResult,
};
use tentacle::{bytes::Bytes, service::TargetSession};

use crate::{
    endpoint::Endpoint,
    error::NetworkError,
    message::NetworkMessage,
    traits::{Compression, MessageSender},
    Context,
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

    fn send(&self, _ctx: Context, msg: Bytes, pri: Priority) -> Result<(), NetworkError> {
        let compressed_msg = self.compression.compress(msg)?;
        self.sender.send(TargetSession::All, compressed_msg, pri)
    }
}

#[async_trait]
impl<S, C> Gossip for NetworkGossip<S, C>
where
    S: MessageSender + Sync + Send + Clone,
    C: Compression + Sync + Send + Clone,
{
    async fn broadcast<M>(
        &self,
        cx: Context,
        end: &str,
        mut msg: M,
        p: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        let endpoint = end.parse::<Endpoint>()?;
        let data = msg.encode().await?;
        let net_msg = NetworkMessage::new(endpoint, data).encode().await?;

        self.send(cx, net_msg, p)?;

        Ok(())
    }
}
