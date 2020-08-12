mod router;

use std::convert::TryFrom;
use std::marker::PhantomData;

use async_trait::async_trait;
use protocol::traits::{Context, MessageCodec, MessageHandler, TrustFeedback};
use protocol::{Bytes, ProtocolResult};

use crate::endpoint::{Endpoint, EndpointScheme, RpcEndpoint};
use crate::message::NetworkMessage;
use crate::rpc::RpcResponse;
use crate::traits::NetworkContext;

pub(crate) use router::{MessageRouter, RemotePeer, RouterContext};

#[async_trait]
pub trait Reactor: Send + Sync {
    async fn react(
        &self,
        context: RouterContext,
        endpoint: Endpoint,
        network_message: NetworkMessage,
    ) -> ProtocolResult<()>;
}

pub struct MessageReactor<M: MessageCodec, H: MessageHandler<Message = M>> {
    msg_handler: H,
}

pub fn generate<M: MessageCodec, H: MessageHandler<Message = M>>(h: H) -> MessageReactor<M, H> {
    MessageReactor { msg_handler: h }
}

pub fn rpc_resp<M: MessageCodec>() -> MessageReactor<M, NoopHandler<M>> {
    MessageReactor {
        msg_handler: NoopHandler::new(),
    }
}

#[async_trait]
impl<M: MessageCodec, H: MessageHandler<Message = M>> Reactor for MessageReactor<M, H> {
    async fn react(
        &self,
        context: RouterContext,
        endpoint: Endpoint,
        network_message: NetworkMessage,
    ) -> ProtocolResult<()> {
        let ctx = Context::new()
            .set_session_id(context.remote_peer.session_id)
            .set_remote_peer_id(context.remote_peer.peer_id.clone())
            .set_remote_connected_addr(context.remote_peer.connected_addr.clone());

        let mut ctx = match (network_message.trace_id(), network_message.span_id()) {
            (Some(trace_id), Some(span_id)) => {
                let span_state = common_apm::muta_apm::MutaTracer::new_state(trace_id, span_id);
                common_apm::muta_apm::MutaTracer::inject_span_state(ctx, span_state)
            }
            _ => ctx,
        };

        let session_id = context.remote_peer.session_id;
        let raw_context = Bytes::from(network_message.content);
        let feedback = match endpoint.scheme() {
            EndpointScheme::Gossip => {
                let content = M::decode(raw_context).await?;
                self.msg_handler.process(ctx, content).await
            }
            EndpointScheme::RpcCall => {
                let content = M::decode(raw_context).await?;
                let rpc_endpoint = RpcEndpoint::try_from(endpoint)?;

                let ctx = ctx.set_rpc_id(rpc_endpoint.rpc_id().value());
                self.msg_handler.process(ctx, content).await
            }
            EndpointScheme::RpcResponse => {
                let content = RpcResponse::decode(raw_context).await?;
                let rpc_endpoint = RpcEndpoint::try_from(endpoint)?;
                let rpc_id = rpc_endpoint.rpc_id().value();

                if !context.rpc_map.contains(session_id, rpc_id) {
                    let full_url = rpc_endpoint.endpoint().full_url();

                    log::warn!(
                        "rpc to {} from {} not found, maybe timeout",
                        full_url,
                        context.remote_peer
                    );
                    return Ok(());
                }

                let rpc_id = rpc_endpoint.rpc_id().value();
                let resp_tx = context.rpc_map.take::<RpcResponse>(session_id, rpc_id)?;
                if resp_tx.send(content).is_err() {
                    let end = rpc_endpoint.endpoint().full_url();
                    log::warn!("network: reactor: {} rpc dropped on {}", session_id, end);
                }

                return Ok(());
            }
        };

        context.report_feedback(feedback);
        Ok(())
    }
}

#[derive(Debug)]
pub struct NoopHandler<M> {
    pin_m: PhantomData<fn() -> M>,
}

impl<M> NoopHandler<M>
where
    M: MessageCodec,
{
    pub fn new() -> Self {
        NoopHandler { pin_m: PhantomData }
    }
}

#[async_trait]
impl<M> MessageHandler for NoopHandler<M>
where
    M: MessageCodec,
{
    type Message = M;

    async fn process(&self, _: Context, _: Self::Message) -> TrustFeedback {
        TrustFeedback::Neutral
    }
}
