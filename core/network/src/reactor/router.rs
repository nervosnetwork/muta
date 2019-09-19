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
use log::{debug, error, warn};
use parking_lot::RwLock;

use crate::{
    endpoint::Endpoint,
    error::{ErrorKind, NetworkError},
    message::{NetworkMessage, RawSessionMessage, SessionMessage},
    traits::Compression,
};

pub struct MessageRouter<C> {
    // Endpoint to reactor channel map
    reactor_map: Arc<RwLock<HashMap<Endpoint, UnboundedSender<SessionMessage>>>>,

    // Receiver for compressed session message
    raw_msg_rx: UnboundedReceiver<RawSessionMessage>,

    // Compression to decompress message
    compression: C,

    // Fatal system error reporter
    sys_tx: UnboundedSender<NetworkError>,
}

impl<C> MessageRouter<C>
where
    C: Compression + Send + Unpin + Clone + 'static,
{
    pub fn new(
        raw_msg_rx: UnboundedReceiver<RawSessionMessage>,
        compression: C,
        sys_tx: UnboundedSender<NetworkError>,
    ) -> Self {
        MessageRouter {
            reactor_map: Default::default(),

            raw_msg_rx,
            compression,

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
        let sys_tx = self.sys_tx.clone();

        let route = async move {
            let des_msg = compression.decompress(raw_msg.msg)?;
            let net_msg = NetworkMessage::decode(des_msg).await?;

            let reactor_map = reactor_map.read();
            let endpoint = net_msg.url.parse::<Endpoint>()?;

            let opt_smsg_tx = reactor_map.get(&endpoint).cloned();
            let smsg_tx = opt_smsg_tx.ok_or_else(|| ErrorKind::NoReactor(endpoint.root()))?;

            let smsg = SessionMessage {
                sid: raw_msg.sid,
                msg: net_msg,
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

impl<C> Future for MessageRouter<C>
where
    C: Compression + Send + Unpin + Clone + 'static,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        debug!("network: router service polled");

        loop {
            let raw_msg_rx = &mut self.as_mut().raw_msg_rx;
            pin_mut!(raw_msg_rx);

            // service ready in common
            let raw_msg = crate::service_ready!("router service", raw_msg_rx.poll_next(ctx));

            runtime::spawn(self.route_raw_message(raw_msg));
        }

        Poll::Pending
    }
}
