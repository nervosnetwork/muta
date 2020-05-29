use std::{sync::Arc, time::Instant};

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
    message::{Headers, NetworkMessage},
    rpc::{RpcErrorMessage, RpcResponse, RpcResponseCode},
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
        let done_rx = self.map.insert::<RpcResponse>(sid, rid);
        let inst = Instant::now();

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
        let mut headers = Headers::default();
        if let Some(state) = common_apm::muta_apm::MutaTracer::span_state(&cx) {
            headers.set_trace_id(state.trace_id());
            headers.set_span_id(state.span_id());
            log::info!("no trace id found for rpc {}", endpoint.full_url());
        }
        common_apm::metrics::network::NETWORK_MESSAGE_COUNT_VEC
            .with_label_values(&["sent", endpoint.full_url()])
            .inc();
        let net_msg = NetworkMessage::new(endpoint, data, headers)
            .encode()
            .await?;

        self.send(cx, sid, net_msg, p)?;

        let timeout = Delay::new(self.timeout.rpc);
        let ret = match future::select(done_rx, timeout).await {
            Either::Left((ret, _timeout)) => {
                ret.map_err(|_| NetworkError::from(ErrorKind::RpcDropped(connected_addr)))?
            }
            Either::Right((_unresolved, _timeout)) => {
                common_apm::metrics::network::NETWORK_RPC_RESULT_COUNT_VEC_STATIC
                    .timeout
                    .inc();

                return Err(NetworkError::from(ErrorKind::RpcTimeout(connected_addr)).into());
            }
        };

        match ret {
            RpcResponse::Success(v) => {
                common_apm::metrics::network::NETWORK_RPC_RESULT_COUNT_VEC_STATIC
                    .success
                    .inc();
                common_apm::metrics::network::NETWORK_PROTOCOL_TIME_HISTOGRAM_VEC_STATIC
                    .rpc
                    .observe(common_apm::metrics::duration_to_sec(inst.elapsed()));

                Ok(R::decode(v).await?)
            }
            RpcResponse::Error(e) => Err(NetworkError::RemoteResponse(Box::new(e)).into()),
        }
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
        let endpoint = end.parse::<Endpoint>()?;
        let sid = cx.session_id()?;
        let rid = cx.rpc_id()?;

        let mut resp = match ret.map_err(|e| e.to_string()) {
            Ok(mut m) => RpcResponse::Success(m.encode().await?),
            Err(err_msg) => RpcResponse::Error(RpcErrorMessage {
                code: RpcResponseCode::ServerError,
                msg:  err_msg,
            }),
        };

        let encoded_resp = resp.encode().await?;
        let endpoint = endpoint.extend(&rid.to_string())?;
        let mut headers = Headers::default();
        if let Some(state) = common_apm::muta_apm::MutaTracer::span_state(&cx) {
            headers.set_trace_id(state.trace_id());
            headers.set_span_id(state.span_id());
            log::info!("no trace id found for rpc {}", endpoint.full_url());
        }
        common_apm::metrics::network::NETWORK_MESSAGE_COUNT_VEC
            .with_label_values(&["sent", endpoint.full_url()])
            .inc();
        let net_msg = NetworkMessage::new(endpoint, encoded_resp, headers)
            .encode()
            .await?;

        self.send(cx, sid, net_msg, p)?;

        Ok(())
    }
}
