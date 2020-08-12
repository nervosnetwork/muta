use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use derive_more::Display;
use futures::channel::mpsc::UnboundedSender;
use parking_lot::RwLock;
use protocol::traits::{MessageCodec, MessageHandler, TrustFeedback};
use protocol::ProtocolResult;
use tentacle::context::ProtocolContextMutRef;
use tentacle::secio::PeerId;
use tentacle::SessionId;

use crate::common::ConnectedAddr;
use crate::endpoint::Endpoint;
use crate::error::{ErrorKind, NetworkError};
use crate::event::PeerManagerEvent;
use crate::message::NetworkMessage;
use crate::protocols::ReceivedMessage;
use crate::rpc_map::RpcMap;
use crate::traits::Compression;

use super::Reactor;

#[derive(Debug, Display)]
#[display(fmt = "connection isnt encrypted, no peer id")]
pub struct NoEncryption {}

#[derive(Debug, Display, Clone)]
#[display(fmt = "remote peer {:?} addr {}", peer_id, connected_addr)]
pub struct RemotePeer {
    pub session_id:     SessionId,
    pub peer_id:        PeerId,
    pub connected_addr: ConnectedAddr,
}

impl RemotePeer {
    pub fn from_proto_context(
        protocol_context: &ProtocolContextMutRef,
    ) -> Result<Self, NoEncryption> {
        let session = protocol_context.session;
        let pubkey = session
            .remote_pubkey
            .as_ref()
            .ok_or_else(|| NoEncryption {})?;

        Ok(RemotePeer {
            session_id:     session.id,
            peer_id:        pubkey.peer_id(),
            connected_addr: ConnectedAddr::from(&session.address),
        })
    }
}

pub struct RouterContext {
    pub(crate) remote_peer: RemotePeer,
    pub(crate) rpc_map:     Arc<RpcMap>,
    trust_tx:               UnboundedSender<PeerManagerEvent>,
}

impl RouterContext {
    fn new(
        remote_peer: RemotePeer,
        rpc_map: Arc<RpcMap>,
        trust_tx: UnboundedSender<PeerManagerEvent>,
    ) -> Self {
        RouterContext {
            remote_peer,
            rpc_map,
            trust_tx,
        }
    }

    pub fn report_feedback(&self, feedback: TrustFeedback) {
        let feedback_event = PeerManagerEvent::TrustMetric {
            pid: self.remote_peer.peer_id.clone(),
            feedback,
        };
        if let Err(e) = self.trust_tx.unbounded_send(feedback_event) {
            log::error!("send peer {} feedback failed {}", self.remote_peer, e);
        }
    }
}

#[derive(Clone)]
pub struct MessageRouter<C> {
    // Endpoint to reactor channel map
    reactor_map: Arc<RwLock<HashMap<Endpoint, Arc<Box<dyn Reactor>>>>>,

    // Rpc map
    pub(crate) rpc_map: Arc<RpcMap>,

    // Sender for peer trust metric feedback
    trust_tx: UnboundedSender<PeerManagerEvent>,

    // Compression to decompress message
    compression: C,
}

impl<C> MessageRouter<C>
where
    C: Compression + Send + Clone + 'static,
{
    pub fn new(
        rpc_map: Arc<RpcMap>,
        trust_tx: UnboundedSender<PeerManagerEvent>,
        compression: C,
    ) -> Self {
        MessageRouter {
            reactor_map: Default::default(),

            rpc_map,
            trust_tx,
            compression,
        }
    }

    pub fn register_reactor<M: MessageCodec>(
        &self,
        endpoint: Endpoint,
        message_handler: impl MessageHandler<Message = M>,
    ) {
        let reactor = super::generate(message_handler);
        self.reactor_map
            .write()
            .insert(endpoint, Arc::new(Box::new(reactor)));
    }

    pub fn register_rpc_response(&self, endpoint: Endpoint) {
        let reactor = super::rpc_resp::<()>();
        self.reactor_map
            .write()
            .insert(endpoint, Arc::new(Box::new(reactor)));
    }

    pub fn route_message(
        &self,
        remote_peer: RemotePeer,
        recv_msg: ReceivedMessage,
    ) -> impl Future<Output = ProtocolResult<()>> {
        let reactor_map = Arc::clone(&self.reactor_map);
        let compression = self.compression.clone();
        let router_context = RouterContext::new(
            remote_peer,
            Arc::clone(&self.rpc_map),
            self.trust_tx.clone(),
        );

        async move {
            let network_message = {
                let decompressed = compression.decompress(recv_msg.data)?;
                NetworkMessage::decode(decompressed)?
            };
            common_apm::metrics::network::on_network_message_received(&network_message.url);

            let endpoint = network_message.url.parse::<Endpoint>()?;
            let reactor = {
                let opt_reactor = reactor_map.read().get(&endpoint).cloned();
                opt_reactor
                    .ok_or_else(|| NetworkError::from(ErrorKind::NoReactor(endpoint.root())))?
            };

            if let Err(err) = reactor
                .react(router_context, endpoint.clone(), network_message)
                .await
            {
                log::error!("process {} message failed: {}", endpoint, err);

                Err(err)
            } else {
                Ok(())
            }
        }
    }
}
