use std::time::Instant;

use async_trait::async_trait;
use futures::future::{self, Either};
use futures_timer::Delay;
use protocol::traits::{Context, MessageCodec, Priority, Rpc};
use protocol::{Bytes, ProtocolResult};
use tentacle::service::TargetSession;
use tentacle::SessionId;

use crate::config::TimeoutConfig;
use crate::endpoint::Endpoint;
use crate::error::{ErrorKind, NetworkError};
use crate::message::{Headers, NetworkMessage};
use crate::protocols::{Recipient, Transmitter, TransmitterMessage};
use crate::rpc::{RpcErrorMessage, RpcResponse, RpcResponseCode};
use crate::traits::{Compression, NetworkContext};

#[derive(Clone)]
pub struct NetworkRpc {
    transmitter: Transmitter,
    timeout:     TimeoutConfig,
}

impl NetworkRpc {
    pub fn new(transmitter: Transmitter, timeout: TimeoutConfig) -> Self {
        NetworkRpc {
            transmitter,
            timeout,
        }
    }

    async fn send(
        &self,
        ctx: Context,
        session_id: SessionId,
        data: Bytes,
        priority: Priority,
    ) -> Result<(), NetworkError> {
        let compressed_data = self.transmitter.compressor().compress(data)?;

        let msg = TransmitterMessage {
            recipient: Recipient::Session(TargetSession::Single(session_id)),
            priority,
            data: compressed_data,
            ctx,
        };

        self.transmitter.behaviour.send(msg).await
    }
}

#[async_trait]
impl Rpc for NetworkRpc {
    async fn call<M, R>(
        &self,
        mut cx: Context,
        endpoint: &str,
        mut msg: M,
        priority: Priority,
    ) -> ProtocolResult<R>
    where
        M: MessageCodec,
        R: MessageCodec,
    {
        let endpoint = endpoint.parse::<Endpoint>()?;
        let sid = cx.session_id()?;
        let rpc_map = &self.transmitter.router.rpc_map;
        let rid = rpc_map.next_rpc_id();
        let connected_addr = cx.remote_connected_addr();
        let done_rx = rpc_map.insert::<RpcResponse>(sid, rid);
        let inst = Instant::now();

        struct _Guard {
            transmitter: Transmitter,
            sid:         SessionId,
            rid:         u64,
        }

        impl Drop for _Guard {
            fn drop(&mut self) {
                // Simple take then drop if there is one
                let rpc_map = &self.transmitter.router.rpc_map;
                let _ = rpc_map.take::<RpcResponse>(self.sid, self.rid);
            }
        }

        let _guard = _Guard {
            transmitter: self.transmitter.clone(),
            sid,
            rid,
        };

        let data = msg.encode()?;
        let endpoint = endpoint.extend(&rid.to_string())?;
        let mut headers = Headers::default();
        if let Some(state) = common_apm::muta_apm::MutaTracer::span_state(&cx) {
            headers.set_trace_id(state.trace_id());
            headers.set_span_id(state.span_id());
            log::info!("no trace id found for rpc {}", endpoint.full_url());
        }
        common_apm::metrics::network::on_network_message_sent(endpoint.full_url());

        let ctx = cx.set_url(endpoint.full_url().to_owned());
        let net_msg = NetworkMessage::new(endpoint, data, headers).encode()?;
        self.send(ctx, sid, net_msg, priority).await?;

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

                Ok(R::decode(v)?)
            }
            RpcResponse::Error(e) => Err(NetworkError::RemoteResponse(Box::new(e)).into()),
        }
    }

    async fn response<M>(
        &self,
        mut cx: Context,
        endpoint: &str,
        ret: ProtocolResult<M>,
        priority: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        let endpoint = endpoint.parse::<Endpoint>()?;
        let sid = cx.session_id()?;
        let rid = cx.rpc_id()?;

        let mut resp = match ret.map_err(|e| e.to_string()) {
            Ok(mut m) => RpcResponse::Success(m.encode()?),
            Err(err_msg) => RpcResponse::Error(RpcErrorMessage {
                code: RpcResponseCode::ServerError,
                msg:  err_msg,
            }),
        };

        let encoded_resp = resp.encode()?;
        let endpoint = endpoint.extend(&rid.to_string())?;
        let mut headers = Headers::default();
        if let Some(state) = common_apm::muta_apm::MutaTracer::span_state(&cx) {
            headers.set_trace_id(state.trace_id());
            headers.set_span_id(state.span_id());
            log::info!("no trace id found for rpc {}", endpoint.full_url());
        }
        common_apm::metrics::network::on_network_message_sent(endpoint.full_url());

        let ctx = cx.set_url(endpoint.full_url().to_owned());
        let net_msg = NetworkMessage::new(endpoint, encoded_resp, headers).encode()?;
        self.send(ctx, sid, net_msg, priority).await?;

        Ok(())
    }
}
