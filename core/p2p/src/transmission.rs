use bytes::Bytes;
use futures::prelude::{Async, Future, Poll, Stream};
use futures::sync::mpsc::{channel, Receiver, SendError, Sender};
use futures::{stream, task};
use log::{debug, info, warn};
use parking_lot::RwLock;
use tentacle::context::{ProtocolContext, ProtocolContextMutRef, ServiceContext};
use tentacle::service::TargetSession;
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::{
    builder::MetaBuilder, multiaddr::Multiaddr, secio::PeerId, traits::ServiceProtocol, ProtocolId,
    SessionId,
};

use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::Send;
use std::sync::Arc;

mod codec;
pub(crate) mod task_handle;

pub use codec::Codec;
use task_handle::{TaskHandle, RECV_DATA_TASK_ID};

/// Protocol name (handshake)
pub const PROTOCOL_NAME: &str = "transmission";

/// Protocol support versions
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

/// Channel buffer size
pub const CHANNEL_BUFFERS: usize = 8;

/// Low-level transport data type
pub type RawMessage = Bytes;

/// The enum for session misbehavior
pub enum Misbehavior {
    /// Send invalid message that cannot be decoded
    InvalidMessage,
}

/// The enum for session misbehavior result
pub enum MisbehaviorResult {
    /// Keep session connection
    Continue,
    /// Disconnect session
    Disconnect,
}

/// Peer manager for transmission protocol
pub trait PeerManager {
    /// Report misbehave to manager
    fn misbehave(
        &mut self,
        peer_id: Option<PeerId>,
        addr: Multiaddr,
        kind: Misbehavior,
    ) -> MisbehaviorResult;
}

/// The enum for sending message to different scoped session[s]
#[derive(Debug)]
pub enum CastMessage<TMessage> {
    /// Send message to given session
    Uni {
        /// Session id
        session_id: SessionId,
        /// Message to sent
        msg: TMessage,
    },

    /// Send message to scoped sessions
    Multi {
        /// Session id
        session_ids: Vec<SessionId>,
        /// Message to sent
        msg: TMessage,
    },

    /// Send message to all connected sessions
    All(TMessage),
}

#[derive(Debug, Clone)]
pub struct RecvMessage<TMessage> {
    session_id: SessionId,
    msg:        TMessage,
}

impl<TMessage> RecvMessage<TMessage> {
    pub fn new(session_id: SessionId, msg: TMessage) -> Self {
        RecvMessage { session_id, msg }
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn take_msg(self) -> TMessage {
        self.msg
    }
}

/// Protocol for datagram transport
pub struct TransmissionProtocol<TMessage, TPeerManager> {
    // Inner protocol id
    id: ProtocolId,

    // Sender for received message from connected sessions
    recv_tx: Sender<RecvMessage<TMessage>>,

    // Receiver for message ready to broadcast to connected sessions
    cast_rx: Arc<RwLock<Receiver<CastMessage<TMessage>>>>,

    // Peer manager for misbehave report
    peer_mgr: TPeerManager,

    // Received data ready to send later
    pending_recv_data: Arc<RwLock<VecDeque<RecvMessage<TMessage>>>>,

    // Stream task handle to pending stream for later notify
    pending_task_handle: TaskHandle,
}

impl<TMessage, TPeerManager> TransmissionProtocol<TMessage, TPeerManager>
where
    TMessage: Codec + Send + Sync + 'static + Debug,
    TPeerManager: PeerManager + Send + Sync + Clone + 'static,
{
    /// Build a TransmissionProtocol instance
    pub fn build(
        id: ProtocolId,
        peer_mgr: TPeerManager,
    ) -> (
        ProtocolMeta,
        Sender<CastMessage<TMessage>>,
        Receiver<RecvMessage<TMessage>>,
    ) {
        let (cast_tx, cast_rx) = channel(CHANNEL_BUFFERS);
        let (recv_tx, recv_rx) = channel(CHANNEL_BUFFERS);

        //
        let cast_rx = Arc::new(RwLock::new(cast_rx));
        let meta = MetaBuilder::default()
            .id(id)
            .name(name!(PROTOCOL_NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || {
                let proto = TransmissionProtocol {
                    id,
                    peer_mgr: peer_mgr.clone(),

                    recv_tx: recv_tx.clone(),
                    cast_rx: Arc::clone(&cast_rx),

                    pending_recv_data: Default::default(),
                    pending_task_handle: Default::default(),
                };

                ProtocolHandle::Callback(Box::new(proto))
            })
            .build();

        (meta, cast_tx, recv_rx)
    }

    pub(crate) fn recv_deliver_task(
        recv_tx: Sender<RecvMessage<TMessage>>,
        pending: Arc<RwLock<VecDeque<RecvMessage<TMessage>>>>,
        task_handle: TaskHandle,
    ) -> Box<dyn Future<Item = (), Error = ()> + Send + 'static> {
        let pending_cloned = Arc::clone(&pending);
        let mut task_handle_cloned = task_handle.clone();

        // create stream from pending
        let pending_stream = stream::poll_fn(
            move || -> Poll<Option<RecvMessage<TMessage>>, SendError<RecvMessage<TMessage>>> {
                // record task handle
                task_handle_cloned.insert(RECV_DATA_TASK_ID, task::current());

                // do poll
                Ok(pending_cloned
                    .write()
                    .pop_front()
                    .map_or(Async::NotReady, |msg| Async::Ready(Some(msg))))
            },
        );

        let deliver_task = pending_stream.forward(recv_tx).then(|finish| {
            if let Err(err) = finish {
                warn!("protocol [transmission]: deliver task error: [{:?}]", err);
                Err(())?
            }
            Ok(())
        });

        Box::new(deliver_task)
    }

    /// Init callback method for ServiceProtocol trait
    pub(crate) fn do_init(&mut self, serv_ctx: &mut ServiceContext) {
        info!("protocol [transmission{}]: do init", self.id);

        let recv_tx = self.recv_tx.clone();
        let pending_recv_data = Arc::clone(&self.pending_recv_data);
        let task_handle = self.pending_task_handle.clone();

        let deliver_task = Self::recv_deliver_task(recv_tx, pending_recv_data, task_handle);
        serv_ctx.future_task(deliver_task)
    }

    pub(crate) fn do_recv(&mut self, proto_ctx: ProtocolContextMutRef, data: RawMessage) {
        let session = proto_ctx.session;
        debug!(
            "protocol [transmission]: message from session [{:?}]",
            (session.id, &session.address, &session.remote_pubkey)
        );

        if let Err(()) = <TMessage as Codec>::decode(&data).and_then(|data| {
            self.pending_recv_data
                .write()
                .push_back(RecvMessage::new(session.id, data));
            self.pending_task_handle.notify(RECV_DATA_TASK_ID);

            Ok(())
        }) {
            let peer_mgr = &mut self.peer_mgr;
            let opt_peer_id = session.remote_pubkey.as_ref().map(PeerId::from_public_key);
            let session_addr = session.address.clone();

            match peer_mgr.misbehave(opt_peer_id, session_addr, Misbehavior::InvalidMessage) {
                MisbehaviorResult::Disconnect => proto_ctx.disconnect(session.id),
                MisbehaviorResult::Continue => (),
            }
        }
    }

    pub(crate) fn do_cast(&mut self, proto_ctx: &mut ProtocolContext) {
        let unpark_cast = |cast_msg: CastMessage<TMessage>| -> (TargetSession, RawMessage) {
            let (target_session, msg) = match cast_msg {
                CastMessage::Uni { session_id, msg } => (TargetSession::Single(session_id), msg),
                CastMessage::Multi { session_ids, msg } => (TargetSession::Multi(session_ids), msg),
                CastMessage::All(msg) => (TargetSession::All, msg),
            };
            (target_session, Codec::encode(msg))
        };

        loop {
            match self.cast_rx.write().poll() {
                Ok(Async::Ready(Some(cast))) => {
                    let (target_session, msg) = unpark_cast(cast);
                    proto_ctx.filter_broadcast(target_session, self.id, msg);
                }
                Ok(Async::NotReady) | Ok(Async::Ready(None)) => break,
                Err(e) => {
                    log::error!("do cast {:?}", e);
                    break;
                }
            }
        }
    }
}

impl<TMessage, TPeerManager> ServiceProtocol for TransmissionProtocol<TMessage, TPeerManager>
where
    TMessage: Codec + Send + Sync + 'static + Debug,
    TPeerManager: PeerManager + Send + Sync + Clone + 'static,
{
    fn init(&mut self, proto_ctx: &mut ProtocolContext) {
        self.do_init(proto_ctx)
    }

    fn received(&mut self, proto_ctx: ProtocolContextMutRef, data: RawMessage) {
        self.do_recv(proto_ctx, data)
    }

    fn poll(&mut self, proto_ctx: &mut ProtocolContext) {
        self.do_cast(proto_ctx)
    }
}
