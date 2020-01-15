use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::future::{self, Either};
use futures_timer::Delay;
use protocol::{
    traits::{Context, MessageCodec, Priority, Rpc},
    Bytes, ProtocolResult,
};
use tentacle::{service::TargetSession, SessionId};

use crate::{
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
    message::NetworkMessage,
    rpc_map::RpcMap,
    traits::{Compression, MessageSender, NetworkContext},
};

const RPC_TIMEOUT: u64 = 4;

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
    S: MessageSender + Send + Sync + Clone,
    C: Compression + Send + Sync + Clone,
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

        // FIXME: timeout from context
        let timeout = Delay::new(Duration::from_secs(RPC_TIMEOUT));
        let ret = match future::select(done_rx, timeout).await {
            Either::Left((ret, _timeout)) => {
                ret.map_err(|_| NetworkError::from(ErrorKind::RpcDropped))?
            }
            Either::Right((_unresolved, _timeout)) => {
                return Err(NetworkError::from(ErrorKind::RpcTimeout).into());
            }
        };

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
