use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    future::TryFutureExt,
    pin_mut,
    stream::Stream,
};
use log::{error, warn};
use parking_lot::RwLock;

use crate::{
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
    event::PeerManagerEvent,
    message::{NetworkMessage, RawSessionMessage, SessionMessage},
    traits::{Compression, SessionBook},
};

pub struct MessageRouter<C, S> {
    // Endpoint to reactor channel map
    reactor_map: Arc<RwLock<HashMap<Endpoint, UnboundedSender<SessionMessage>>>>,

    // Receiver for compressed session message
    raw_msg_rx: UnboundedReceiver<RawSessionMessage>,

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
        raw_msg_rx: UnboundedReceiver<RawSessionMessage>,
        trust_tx: UnboundedSender<PeerManagerEvent>,
        compression: C,
        sessions: S,
        sys_tx: UnboundedSender<NetworkError>,
    ) -> Self {
        MessageRouter {
            reactor_map: Default::default(),

            raw_msg_rx,
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

    pub fn route_raw_message(&self, raw_msg: RawSessionMessage) -> impl Future<Output = ()> {
        let reactor_map = Arc::clone(&self.reactor_map);
        let compression = self.compression.clone();
        let sessions = self.sessions.clone();
        let sys_tx = self.sys_tx.clone();
        let trust_tx = self.trust_tx.clone();

        let route = async move {
            let des_msg = compression.decompress(raw_msg.msg)?;
            let net_msg = NetworkMessage::decode(des_msg).await?;
            common_apm::metrics::network::on_network_message_received(&net_msg.url);

            let reactor_map = reactor_map.read();
            let endpoint = net_msg.url.parse::<Endpoint>()?;

            let opt_smsg_tx = reactor_map.get(&endpoint).cloned();
            let smsg_tx = opt_smsg_tx.ok_or_else(|| ErrorKind::NoReactor(endpoint.root()))?;

            // Peer may disconnect when we try to fetch its connected address.
            // This connected addr is mainly for debug purpose, so no error.
            let connected_addr = sessions.connected_addr(raw_msg.sid);
            let smsg = SessionMessage {
                sid: raw_msg.sid,
                pid: raw_msg.pid,
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
            let raw_msg_rx = &mut self.as_mut().raw_msg_rx;
            pin_mut!(raw_msg_rx);

            // service ready in common
            let raw_msg = crate::service_ready!("router service", raw_msg_rx.poll_next(ctx));

            tokio::spawn(self.route_raw_message(raw_msg));
        }

        Poll::Pending
    }
}
