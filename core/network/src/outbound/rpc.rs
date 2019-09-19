use std::sync::Arc;

use async_trait::async_trait;
use futures::future::TryFutureExt;
use protocol::{
    traits::{MessageCodec, Priority, Rpc},
    ProtocolResult,
};
use tentacle::{bytes::Bytes, service::TargetSession, SessionId};

use crate::{
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
    message::NetworkMessage,
    rpc_map::RpcMap,
    traits::{Compression, MessageSender, NetworkContext},
    Context,
};

#[derive(Clone)]
pub struct NetworkRpc<S, C> {
    sender:      S,
    compression: C,
    map:         Arc<RpcMap>,
}

impl<S, C> NetworkRpc<S, C>
where
    S: MessageSender + Sync + Clone,
    C: Compression + Sync + Clone,
{
    pub fn new(sender: S, compression: C, map: Arc<RpcMap>) -> Self {
        NetworkRpc {
            sender,
            compression,
            map,
        }
    }

    fn send(&self, _: Context, s: SessionId, msg: Bytes, p: Priority) -> Result<(), NetworkError> {
        let compressed_msg = self.compression.compress(msg)?;
        let target = TargetSession::Single(s);

        self.sender.send(target, compressed_msg, p)
    }
}

#[async_trait]
impl<S, C> Rpc for NetworkRpc<S, C>
where
    S: MessageSender + Sync + Clone,
    C: Compression + Sync + Clone,
{
    async fn call<M, R>(&self, cx: Context, end: &str, mut msg: M, p: Priority) -> ProtocolResult<R>
    where
        M: MessageCodec,
        R: MessageCodec,
    {
        let endpoint = end.parse::<Endpoint>()?;
        let sid = cx.session_id()?;
        let rid = self.map.next_rpc_id();
        let done_rx = self.map.insert::<R>(sid, rid);

        let data = msg.encode().await?;
        let endpoint = endpoint.extend(&rid.to_string())?;
        let net_msg = NetworkMessage::new(endpoint, data).encode().await?;

        self.send(cx, sid, net_msg, p)?;

        let ret = done_rx
            .map_err(|_| ErrorKind::RpcDropped)
            .err_into::<NetworkError>()
            .await?;

        Ok(ret)
    }

    async fn response<M>(
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
        let sid = cx.session_id()?;
        let rid = cx.rpc_id()?;

        let data = msg.encode().await?;
        let endpoint = endpoint.extend(&rid.to_string())?;
        let net_msg = NetworkMessage::new(endpoint, data).encode().await?;

        self.send(cx, sid, net_msg, p)?;

        Ok(())
    }
}
