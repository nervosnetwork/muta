mod router;
pub(crate) use router::MessageRouter;

use std::{
    convert::TryFrom,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context as TaskContext, Poll},
};

use async_trait::async_trait;
use futures::{channel::mpsc::UnboundedReceiver, future::TryFutureExt, pin_mut, stream::Stream};
use log::warn;
use protocol::{
    traits::{Context, MessageCodec, MessageHandler},
    Bytes, ProtocolError,
};

use crate::{
    endpoint::{Endpoint, EndpointScheme, RpcEndpoint},
    message::SessionMessage,
    rpc_map::RpcMap,
    traits::NetworkContext,
};

pub struct Reactor<M> {
    smsg_rx: UnboundedReceiver<SessionMessage>,
    handler: Arc<Box<dyn MessageHandler<Message = M>>>,
    rpc_map: Arc<RpcMap>,
}

impl<M> Reactor<M>
where
    M: MessageCodec,
{
    pub fn new(
        smsg_rx: UnboundedReceiver<SessionMessage>,
        boxed_handler: Box<dyn MessageHandler<Message = M>>,
        rpc_map: Arc<RpcMap>,
    ) -> Self {
        Reactor {
            smsg_rx,
            handler: Arc::new(boxed_handler),
            rpc_map,
        }
    }

    pub fn rpc_resp(smsg_rx: UnboundedReceiver<SessionMessage>, rpc_map: Arc<RpcMap>) -> Self {
        Reactor {
            smsg_rx,
            handler: Arc::new(Box::new(DummyHandler::new())),
            rpc_map,
        }
    }

    pub fn react(&self, smsg: SessionMessage) -> impl Future<Output = ()> {
        let handler = Arc::clone(&self.handler);
        let rpc_map = Arc::clone(&self.rpc_map);

        let SessionMessage {
            sid,
            msg: net_msg,
            pid,
            connected_addr,
            ..
        } = smsg;

        let endpoint = net_msg.url.to_owned();
        let mut ctx = Context::new().set_session_id(sid).set_remote_peer_id(pid);
        if let Some(ref connected_addr) = connected_addr {
            ctx = ctx.set_remote_connected_addr(connected_addr.clone());
        }

        let react = async move {
            let endpoint = net_msg.url.parse::<Endpoint>()?;
            let content = M::decode(Bytes::from(net_msg.content)).await?;

            match endpoint.scheme() {
                EndpointScheme::Gossip => handler.process(ctx, content).await,
                EndpointScheme::RpcCall => {
                    let rpc_endpoint = RpcEndpoint::try_from(endpoint)?;

                    let ctx = ctx.set_rpc_id(rpc_endpoint.rpc_id().value());
                    handler.process(ctx, content).await
                }
                EndpointScheme::RpcResponse => {
                    let rpc_endpoint = RpcEndpoint::try_from(endpoint)?;
                    let rpc_id = rpc_endpoint.rpc_id().value();

                    if !rpc_map.contains(sid, rpc_id) {
                        let full_url = rpc_endpoint.endpoint().full_url();

                        warn!(
                            "rpc entry for {} from {:?} not found, maybe timeout",
                            full_url, connected_addr
                        );
                        return Ok(());
                    }

                    let resp_tx = rpc_map.take::<M>(sid, rpc_endpoint.rpc_id().value())?;
                    if resp_tx.send(content).is_err() {
                        let end = rpc_endpoint.endpoint().full_url();

                        warn!("network: reactor: {} rpc dropped on {}", sid, end);
                    }
                }
            }

            Ok::<(), ProtocolError>(())
        };

        react.unwrap_or_else(move |err| warn!("network: {} reactor: {}", endpoint, err))
    }
}

impl<M> Future for Reactor<M>
where
    M: MessageCodec,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        loop {
            let smsg_rx = &mut self.as_mut().smsg_rx;
            pin_mut!(smsg_rx);

            let reactor_name = concat!("reactor service", stringify!(M));
            let smsg = crate::service_ready!(reactor_name, smsg_rx.poll_next(ctx));

            tokio::spawn(self.react(smsg));
        }

        Poll::Pending
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

    async fn process(&self, _: Context, _: Self::Message) {}
}
