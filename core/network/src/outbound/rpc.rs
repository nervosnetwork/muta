use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{self, Either};
use futures_timer::Delay;
use protocol::{
    traits::{Context, MessageCodec, Priority, Rpc},
    Bytes, BytesMut, ProtocolResult,
};
use tentacle::{service::TargetSession, SessionId};

use crate::{
    config::TimeoutConfig,
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
    message::NetworkMessage,
    protocols::{DataMeta, PullError, PullTimeout, PushPull},
    rpc::{RpcErrorMessage, RpcResponse, RpcResponseCode},
    rpc_map::RpcMap,
    traits::{Compression, MessageSender, NetworkContext},
};

#[derive(Clone)]
pub struct NetworkRpc<S, C> {
    sender:      S,
    push_pull:   PushPull,
    compression: C,
    map:         Arc<RpcMap>,

    timeout: TimeoutConfig,
}

impl<S, C> NetworkRpc<S, C>
where
    S: MessageSender + Sync + Clone,
    C: Compression + Sync + Clone,
{
    pub fn new(
        sender: S,
        push_pull: PushPull,
        compression: C,
        map: Arc<RpcMap>,
        timeout: TimeoutConfig,
    ) -> Self {
        NetworkRpc {
            sender,
            push_pull,
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
    S: MessageSender + Send + Sync + Clone + Unpin + 'static,
    C: Compression + Send + Sync + Clone,
{
    async fn call<M, R>(&self, cx: Context, end: &str, mut msg: M, p: Priority) -> ProtocolResult<R>
    where
        M: MessageCodec,
        R: MessageCodec,
    {
        use prost::Message;

        let endpoint = end.parse::<Endpoint>()?;
        let sid = cx.session_id()?;
        let rid = self.map.next_rpc_id();
        let connected_addr = cx.remote_connected_addr();
        let done_rx = self.map.insert::<RpcResponse>(sid, rid);

        struct _Guard {
            map: Arc<RpcMap>,
            sid: SessionId,
            rid: u64,
        }

        impl Drop for _Guard {
            fn drop(&mut self) {
                // Simple take then drop if there is one
                let _ = self.map.take::<RpcResponse>(self.sid, self.rid);
            }
        }

        let _guard = _Guard {
            map: Arc::clone(&self.map),
            sid,
            rid,
        };

        let data = msg.encode().await?;
        let endpoint = endpoint.extend(&rid.to_string())?;
        let net_msg = NetworkMessage::new(endpoint, data).encode().await?;

        self.send(cx, sid, net_msg, p)?;

        let timeout = Delay::new(self.timeout.rpc);
        let ret = match future::select(done_rx, timeout).await {
            Either::Left((ret, _timeout)) => {
                ret.map_err(|_| ErrorKind::RpcDropped(connected_addr.clone()))?
            }
            Either::Right((_unresolved, _timeout)) => {
                return Err(ErrorKind::RpcTimeout(connected_addr).into());
            }
        };

        let data_meta = match ret {
            RpcResponse::Success(v) => {
                DataMeta::decode(v).map_err(|e| NetworkError::SerdeError(Box::new(e)))?
            }
            RpcResponse::Error(e) => return Err(NetworkError::RemoteResponse(Box::new(e)).into()),
        };
        log::debug!("got rpc response data_meta {:?}", data_meta);

        let data_hash = data_meta
            .hash
            .ok_or_else(|| ErrorKind::PullNoDataHash(connected_addr.clone()))?;
        let data_len = data_meta.length;

        let net = self.sender.clone();
        let timeout = PullTimeout {
            chunk: self.timeout.pull_chunk,
            max:   self.timeout.pull_max,
        };

        let fut_pull = self
            .push_pull
            .pull(net, sid, timeout, data_hash.clone(), data_len)
            .map_err(|e| ErrorKind::PullInternal {
                remote: connected_addr.clone(),
                cause:  Box::new(e),
            })?;

        let data = fut_pull.await.map_err(|e| {
            let data_hash = data_hash.to_string();
            let remote = connected_addr;

            match &e {
                PullError::Timeout => ErrorKind::PullTimeout { data_hash, remote },
                PullError::NotFound => ErrorKind::PullNotFound { data_hash, remote },
                PullError::Internal(_) => ErrorKind::PullInternal {
                    remote,
                    cause: Box::new(e),
                },
            }
        })?;

        Ok(R::decode(data).await?)
    }

    async fn response<M>(
        &self,
        cx: Context,
        end: &str,
        ret: ProtocolResult<M>,
        p: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        use prost::Message;

        let endpoint = end.parse::<Endpoint>()?;
        let sid = cx.session_id()?;
        let rid = cx.rpc_id()?;

        let mut resp = match ret.map_err(|e| e.to_string()) {
            Ok(mut m) => {
                let data = m.encode().await?;
                let data_meta = self.push_pull.cache_data(data);
                log::debug!("cache rpc response, data_meta {:?}", data_meta);

                let mut buf = BytesMut::with_capacity(data_meta.encoded_len());
                data_meta
                    .encode(&mut buf)
                    .map_err(|e| NetworkError::SerdeError(Box::new(e)))?;
                RpcResponse::Success(buf.freeze())
            }
            Err(err_msg) => RpcResponse::Error(RpcErrorMessage {
                code: RpcResponseCode::ServerError,
                msg:  err_msg,
            }),
        };

        let encoded_resp = resp.encode().await?;
        let endpoint = endpoint.extend(&rid.to_string())?;
        let net_msg = NetworkMessage::new(endpoint, encoded_resp).encode().await?;

        self.send(cx, sid, net_msg, p)?;

        Ok(())
    }
}
