use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::future::TryFutureExt;
use futures::stream::Stream;
use log::{error, warn};
use parking_lot::RwLock;

use crate::endpoint::Endpoint;
use crate::error::{ErrorKind, NetworkError};
use crate::event::PeerManagerEvent;
use crate::message::{NetworkMessage, SessionMessage};
use crate::protocols::ReceivedMessage;
use crate::traits::{Compression, SessionBook};

pub struct MessageRouter<C, S> {
    // Endpoint to reactor channel map
    reactor_map: Arc<RwLock<HashMap<Endpoint, UnboundedSender<SessionMessage>>>>,

    // Receiver for compressed session message
    recv_data_rx: UnboundedReceiver<ReceivedMessage>,

    // Sender for peer trust metric feedback
    trust_tx: UnboundedSender<PeerManagerEvent>,

    // Compression to decompress message
    compression: C,

    // Session book
    sessions: S,

    // Fatal system error reporter
    sys_tx: UnboundedSender<NetworkError>,
}

impl<C, S> MessageRouter<C, S>
where
    C: Compression + Send + Unpin + Clone + 'static,
    S: SessionBook + Send + Unpin + Clone + 'static,
{
    pub fn new(
        recv_data_rx: UnboundedReceiver<ReceivedMessage>,
        trust_tx: UnboundedSender<PeerManagerEvent>,
        compression: C,
        sessions: S,
        sys_tx: UnboundedSender<NetworkError>,
    ) -> Self {
        MessageRouter {
            reactor_map: Default::default(),

            recv_data_rx,
            trust_tx,
            compression,
            sessions,

            sys_tx,
        }
    }

    pub fn register_reactor(
        &mut self,
        endpoint: Endpoint,
        smsg_tx: UnboundedSender<SessionMessage>,
    ) {
        self.reactor_map.write().insert(endpoint, smsg_tx);
    }

    pub fn route_recived_message(&self, recv_msg: ReceivedMessage) -> impl Future<Output = ()> {
        let reactor_map = Arc::clone(&self.reactor_map);
        let compression = self.compression.clone();
        let sessions = self.sessions.clone();
        let sys_tx = self.sys_tx.clone();
        let trust_tx = self.trust_tx.clone();

        let route = async move {
            let des_msg = compression.decompress(recv_msg.data)?;
            let net_msg = NetworkMessage::decode(des_msg).await?;
            common_apm::metrics::network::on_network_message_received(&net_msg.url);

            let reactor_map = reactor_map.read();
            let endpoint = net_msg.url.parse::<Endpoint>()?;

            let opt_smsg_tx = reactor_map.get(&endpoint).cloned();
            let smsg_tx = opt_smsg_tx.ok_or_else(|| ErrorKind::NoReactor(endpoint.root()))?;

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

            if smsg_tx.unbounded_send(smsg).is_err() {
                error!("network: we lost {} reactor", endpoint.root());

                // If network service is offline, there's nothing we can do
                let _ = sys_tx.unbounded_send(ErrorKind::Offline("reactor").into());
            }

            Ok::<(), NetworkError>(())
        };

        route.unwrap_or_else(|err| warn!("network: router {}", err))
    }
}

impl<C, S> Future for MessageRouter<C, S>
where
    C: Compression + Send + Unpin + Clone + 'static,
    S: SessionBook + Send + Unpin + Clone + 'static,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let recv_data_rx = &mut self.as_mut().recv_data_rx;
            futures::pin_mut!(recv_data_rx);

            // service ready in common
            let recv_msg = crate::service_ready!("router service", recv_data_rx.poll_next(ctx));

            tokio::spawn(self.route_recived_message(recv_msg));
        }

        Poll::Pending
    }
}
