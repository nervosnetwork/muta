use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use futures::future::{self, Either};
use futures_timer::Delay;
use protocol::{
    traits::{Context, MessageCodec, Priority, Rpc},
    Bytes, ProtocolResult,
};
use tentacle::{service::TargetSession, SessionId};

use crate::{
    config::TimeoutConfig,
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
    message::NetworkMessage,
    rpc_map::RpcMap,
    traits::{Compression, MessageSender, NetworkContext},
};

#[derive(Clone)]
pub struct NetworkRpc<S, C> {
    sender:      S,
    compression: C,
    map:         Arc<RpcMap>,

    timeout: TimeoutConfig,
}

impl<S, C> NetworkRpc<S, C>
where
    S: MessageSender + Sync + Clone,
    C: Compression + Sync + Clone,
{
    pub fn new(sender: S, compression: C, map: Arc<RpcMap>, timeout: TimeoutConfig) -> Self {
        NetworkRpc {
            sender,
            compression,
            map,

            timeout,
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
        let connected_addr = cx.remote_connected_addr();
        let done_rx = self.map.insert::<R>(sid, rid);

        struct _Guard<R: MessageCodec> {
            map: Arc<RpcMap>,
            sid: SessionId,
            rid: u64,

            _r: PhantomData<R>,
        }

        impl<R: MessageCodec> Drop for _Guard<R> {
            fn drop(&mut self) {
                // Simple take then drop if there is one
                let _ = self.map.take::<R>(self.sid, self.rid);
            }
        }

        let _guard = _Guard::<R> {
            map: Arc::clone(&self.map),
            sid,
            rid,
            _r: PhantomData,
        };

        let data = msg.encode().await?;
        let endpoint = endpoint.extend(&rid.to_string())?;
        let net_msg = NetworkMessage::new(endpoint, data).encode().await?;

        self.send(cx, sid, net_msg, p)?;

        let timeout = Delay::new(self.timeout.rpc);
        let ret = match future::select(done_rx, timeout).await {
            Either::Left((ret, _timeout)) => {
                ret.map_err(|_| NetworkError::from(ErrorKind::RpcDropped(connected_addr)))?
            }
            Either::Right((_unresolved, _timeout)) => {
                return Err(NetworkError::from(ErrorKind::RpcTimeout(connected_addr)).into());
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
