use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::stream::Stream;
use parking_lot::RwLock;
use protocol::traits::{MessageCodec, MessageHandler};
use protocol::ProtocolResult;

use crate::endpoint::Endpoint;
use crate::error::{ErrorKind, NetworkError};
use crate::event::PeerManagerEvent;
use crate::message::{NetworkMessage, SessionMessage};
use crate::protocols::ReceivedMessage;
use crate::rpc_map::RpcMap;
use crate::traits::{Compression, SharedSessionBook};

use super::Reactor;

pub struct MessageRouter<C, S> {
    // Endpoint to reactor channel map
    reactor_map: Arc<RwLock<HashMap<Endpoint, Arc<Box<dyn Reactor>>>>>,

    // Rpc map
    rpc_map: Arc<RpcMap>,

    // Receiver for compressed session message
    recv_data_rx: UnboundedReceiver<ReceivedMessage>,

    // Sender for peer trust metric feedback
    trust_tx: UnboundedSender<PeerManagerEvent>,

    // Compression to decompress message
    compression: C,

    // Session book
    sessions: S,
}

impl<C, S> MessageRouter<C, S>
where
    C: Compression + Send + Unpin + Clone + 'static,
    S: SharedSessionBook + Send + Unpin + Clone + 'static,
{
    pub fn new(
        rpc_map: Arc<RpcMap>,
        recv_data_rx: UnboundedReceiver<ReceivedMessage>,
        trust_tx: UnboundedSender<PeerManagerEvent>,
        compression: C,
        sessions: S,
    ) -> Self {
        MessageRouter {
            reactor_map: Default::default(),

            rpc_map,
            recv_data_rx,
            trust_tx,
            compression,
            sessions,
        }
    }

    pub fn register_reactor<M: MessageCodec>(
        &self,
        endpoint: Endpoint,
        message_handler: impl MessageHandler<Message = M>,
    ) {
        let reactor = super::generate(message_handler, Arc::clone(&self.rpc_map));
        self.reactor_map
            .write()
            .insert(endpoint, Arc::new(Box::new(reactor)));
    }

    pub fn register_rpc_response(&self, endpoint: Endpoint) {
        let reactor = super::rpc_resp::<()>(Arc::clone(&self.rpc_map));
        self.reactor_map
            .write()
            .insert(endpoint, Arc::new(Box::new(reactor)));
    }

    pub fn route_message(
        &self,
        recv_msg: ReceivedMessage,
    ) -> impl Future<Output = ProtocolResult<()>> {
        let reactor_map = Arc::clone(&self.reactor_map);
        let compression = self.compression.clone();
        let sessions = self.sessions.clone();
        let trust_tx = self.trust_tx.clone();

        async move {
            let des_msg = compression.decompress(recv_msg.data)?;
            let net_msg = NetworkMessage::decode(des_msg)?;
            common_apm::metrics::network::on_network_message_received(&net_msg.url);

            let endpoint = net_msg.url.parse::<Endpoint>()?;
            let reactor = {
                let opt_reactor = reactor_map.read().get(&endpoint).cloned();
                opt_reactor
                    .ok_or_else(|| NetworkError::from(ErrorKind::NoReactor(endpoint.root())))?
            };

            // Peer may disconnect when we try to fetch its connected address.
            // This connected addr is mainly for debug purpose, so no error.
            let connected_addr = sessions.connected_addr(recv_msg.session_id);
            let smsg = SessionMessage {
                sid: recv_msg.session_id,
                pid: recv_msg.peer_id,
                msg: net_msg,
                connected_addr,
                trust_tx,
            };

            if let Err(err) = reactor.react(&smsg).await {
                log::error!("process {} message failed: {}", endpoint, err);

                Err(err)
            } else {
                Ok(())
            }
        }
    }
}

impl<C, S> Future for MessageRouter<C, S>
where
    C: Compression + Send + Unpin + Clone + 'static,
    S: SharedSessionBook + Send + Unpin + Clone + 'static,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        loop {
            let recv_data_rx = &mut self.as_mut().recv_data_rx;
            futures::pin_mut!(recv_data_rx);

            // service ready in common
            let recv_msg = crate::service_ready!("router service", recv_data_rx.poll_next(ctx));

            tokio::spawn(self.route_message(recv_msg));
        }

        Poll::Pending
    }
}
