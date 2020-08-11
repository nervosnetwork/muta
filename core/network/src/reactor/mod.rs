mod router;

use std::convert::TryFrom;
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use log::{error, warn};
use protocol::traits::{Context, MessageCodec, MessageHandler, TrustFeedback};
use protocol::{Bytes, ProtocolResult};

use crate::endpoint::{Endpoint, EndpointScheme, RpcEndpoint};
use crate::event::PeerManagerEvent;
use crate::message::SessionMessage;
use crate::rpc::RpcResponse;
use crate::rpc_map::RpcMap;
use crate::traits::NetworkContext;

pub(crate) use router::MessageRouter;

#[async_trait]
pub trait Reactor: Send + Sync {
    async fn react(&self, session_message: &SessionMessage) -> ProtocolResult<()>;
}

pub struct MessageReactor<M: MessageCodec, H: MessageHandler<Message = M>> {
    msg_handler: H,
    rpc_map:     Arc<RpcMap>,
}

pub fn generate<M: MessageCodec, H: MessageHandler<Message = M>>(
    h: H,
    rpc_map: Arc<RpcMap>,
) -> MessageReactor<M, H> {
    MessageReactor {
        msg_handler: h,
        rpc_map,
    }
}

pub fn rpc_resp<M: MessageCodec>(rpc_map: Arc<RpcMap>) -> MessageReactor<M, DummyHandler<M>> {
    MessageReactor {
        msg_handler: DummyHandler::new(),
        rpc_map,
    }
}

#[async_trait]
impl<M: MessageCodec, H: MessageHandler<Message = M>> Reactor for MessageReactor<M, H> {
    async fn react(&self, session_message: &SessionMessage) -> ProtocolResult<()> {
        let mut ctx = Context::new()
            .set_session_id(session_message.sid)
            .set_remote_peer_id(session_message.pid.clone());

        if let Some(ref connected_addr) = session_message.connected_addr {
            ctx = ctx.set_remote_connected_addr(connected_addr.clone());
        }

        let mut ctx = match (
            session_message.msg.trace_id(),
            session_message.msg.span_id(),
        ) {
            (Some(trace_id), Some(span_id)) => {
                let span_state = common_apm::muta_apm::MutaTracer::new_state(trace_id, span_id);
                common_apm::muta_apm::MutaTracer::inject_span_state(ctx, span_state)
            }
            _ => ctx,
        };

        let endpoint = session_message.msg.url.parse::<Endpoint>()?;
        let session_id = session_message.sid;

        let feedback = match endpoint.scheme() {
            EndpointScheme::Gossip => {
                let content = M::decode(Bytes::from(session_message.msg.content.to_vec())).await?;
                self.msg_handler.process(ctx, content).await
            }
            EndpointScheme::RpcCall => {
                let content = M::decode(Bytes::from(session_message.msg.content.to_vec())).await?;
                let rpc_endpoint = RpcEndpoint::try_from(endpoint)?;

                let ctx = ctx.set_rpc_id(rpc_endpoint.rpc_id().value());
                self.msg_handler.process(ctx, content).await
            }
            EndpointScheme::RpcResponse => {
                let content =
                    RpcResponse::decode(Bytes::from(session_message.msg.content.to_vec())).await?;
                let rpc_endpoint = RpcEndpoint::try_from(endpoint)?;
                let rpc_id = rpc_endpoint.rpc_id().value();

                if !self.rpc_map.contains(session_id, rpc_id) {
                    let full_url = rpc_endpoint.endpoint().full_url();

                    warn!(
                        "rpc entry for {} from {:?} not found, maybe timeout",
                        full_url, session_message.connected_addr
                    );
                    return Ok(());
                }

                let resp_tx = self
                    .rpc_map
                    .take::<RpcResponse>(session_id, rpc_endpoint.rpc_id().value())?;
                if resp_tx.send(content).is_err() {
                    let end = rpc_endpoint.endpoint().full_url();

                    warn!("network: reactor: {} rpc dropped on {}", session_id, end);
                }
                return Ok(());
            }
        };

        let trust_feedback = PeerManagerEvent::TrustMetric {
            pid: session_message.pid.clone(),
            feedback,
        };
        if let Err(e) = session_message.trust_tx.unbounded_send(trust_feedback) {
            error!("send peer trust report {}", e);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct DummyHandler<M> {
    pin_m: PhantomData<fn() -> M>,
}

impl<M> DummyHandler<M>
where
    M: MessageCodec,
{
    pub fn new() -> Self {
        DummyHandler { pin_m: PhantomData }
    }
}

#[async_trait]
impl<M> MessageHandler for DummyHandler<M>
where
    M: MessageCodec,
{
    type Message = M;

    async fn process(&self, _: Context, _: Self::Message) -> TrustFeedback {
        TrustFeedback::Neutral
    }
}
