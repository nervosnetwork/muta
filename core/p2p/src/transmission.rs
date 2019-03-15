use bytes::Bytes;
use futures::prelude::{Async, Future, Poll, Stream};
use futures::sync::mpsc::{channel, Receiver, SendError, Sender};
use futures::{future, stream, task};
use log::{debug, info, trace, warn};
use parking_lot::{Mutex, RwLock};
use tentacle::context::{ServiceContext, SessionContext};
use tentacle::service::{ProtocolHandle, ProtocolMeta, ServiceControl, ServiceTask};
use tentacle::{
    builder::MetaBuilder, error::Error, multiaddr::Multiaddr, secio::PeerId,
    traits::ServiceProtocol, ProtocolId, SessionId,
};
use tokio::timer::Delay;

use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::Send;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod codec;
pub(crate) mod task_handle;

pub use codec::Codec;
use task_handle::{TaskHandle, BROADCAST_TASK_ID, RECV_DATA_TASK_ID};

/// Protocol name (handshake)
pub const PROTOCOL_NAME: &str = "transmission";

/// Protocol support versions
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

/// Channel buffer size
pub const CHANNEL_BUFFERS: usize = 8;

/// Cast channel retry delay (seconds)
pub const CHANNEL_CAST_RETRY_DELAY: u64 = 2;

/// Low-level transport data type
pub type RawMessage = Bytes;

/// The enum for session misbehavior
pub enum Misbehavior {
    /// Send invalid message that cannot be decoded
    InvalidMessage,
}

/// Peer manager for transmission protocol
pub trait PeerManager {
    /// Report misbehave to manager
    fn misbehave(&mut self, peer_id: Option<PeerId>, addr: Multiaddr, kind: Misbehavior) -> i32;
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

// Data type for `pending_raw_casts`
type RawCast = (Option<Vec<SessionId>>, RawMessage);

// The enum wrapper for `CastMessage` and `RawCast`
#[derive(Debug)]
enum DoCast<TMessage: Debug> {
    Message(CastMessage<TMessage>),
    Raw(RawCast),
}

/// Protocol for datagram transport
pub struct TransmissionProtocol<TMessage, TPeerManager> {
    // Inner protocol id
    id: ProtocolId,

    // Sender for received message from connected sessions
    recv_tx: Sender<TMessage>,

    // Receiver for message ready to broadcast to connected sessions
    // note: Becasue we need to move it into another thread, must wrap
    // it inside Arc<Mutex<_>>.
    cast_rx: Arc<Mutex<Option<Receiver<CastMessage<TMessage>>>>>,

    // Peer manager for misbehave report
    peer_mgr: TPeerManager,

    // Messages ready to send later
    pending_raw_casts: Arc<RwLock<VecDeque<RawCast>>>,

    // Received data ready to send later
    pending_recv_data: Arc<RwLock<VecDeque<TMessage>>>,

    // Stream task handle to pending stream for later notify
    pending_task_handles: TaskHandle,
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
        Receiver<TMessage>,
    ) {
        let (cast_tx, cast_rx) = channel(CHANNEL_BUFFERS);
        let (recv_tx, recv_rx) = channel::<TMessage>(CHANNEL_BUFFERS);
        let cast_rx = Arc::new(Mutex::new(Some(cast_rx)));

        let support_versions = SUPPORT_VERSIONS
            .to_vec()
            .into_iter()
            .map(String::from)
            .collect();

        let proto = move || -> TransmissionProtocol<TMessage, TPeerManager> {
            TransmissionProtocol {
                id,
                peer_mgr: peer_mgr.clone(),

                recv_tx: recv_tx.clone(),
                cast_rx: Arc::clone(&cast_rx),

                pending_raw_casts: Default::default(),
                pending_recv_data: Default::default(),
                pending_task_handles: Default::default(),
            }
        };

        let meta = MetaBuilder::default()
            .id(id)
            .name(|id| format!("{}/{}", PROTOCOL_NAME, id))
            .support_versions(support_versions)
            .service_handle(move |_meta| ProtocolHandle::Callback(Box::new(proto())))
            .build();

        (meta, cast_tx, recv_rx)
    }

    pub(crate) fn broadcast_task(
        proto_id: ProtocolId,
        cast_rx: Receiver<CastMessage<TMessage>>,
        pending: Arc<RwLock<VecDeque<RawCast>>>,
        task_handle: TaskHandle,
        mut control: ServiceControl,
    ) -> Box<dyn Future<Item = (), Error = ()> + Send + 'static> {
        let pending_cloned = Arc::clone(&pending);
        let mut task_handle_cloned = task_handle.clone();

        // create stream from pending
        let pending_stream = stream::poll_fn(move || -> Poll<Option<DoCast<TMessage>>, ()> {
            // record task handle
            task_handle_cloned.insert(BROADCAST_TASK_ID, task::current());

            // do poll
            Ok(pending_cloned
                .write()
                .pop_front()
                .map_or(Async::NotReady, |raw| Async::Ready(Some(DoCast::Raw(raw)))))
        });

        let unpark_cast = |do_cast: DoCast<TMessage>| -> (Option<Vec<SessionId>>, RawMessage) {
            match do_cast {
                DoCast::Raw(raw_cast) => raw_cast,
                DoCast::Message(cast) => {
                    let (session_ids, msg) = match cast {
                        CastMessage::Uni { session_id, msg } => (Some(vec![session_id]), msg),
                        CastMessage::Multi { session_ids, msg } => (Some(session_ids), msg),
                        CastMessage::All(msg) => (None, msg),
                    };
                    (session_ids, Codec::encode(msg))
                }
            }
        };

        let do_broadcast = move |do_cast: DoCast<TMessage>|
              -> Box<Future<Item = (), Error = ()> + Send + 'static> {

            let (session_ids, raw_msg) = unpark_cast(do_cast);

            if let Err(err) = control.filter_broadcast(session_ids, proto_id, raw_msg.to_vec()) {
                debug!("protocol [transmission]: fail to send message");
                trace!("protocol [transmission]: *** broadcast error ***: {:?}", err);

                // if full then move it to pending
                let task_handle = task_handle.clone();
                if let Error::TaskFull(ServiceTask::ProtocolMessage { session_ids, data, ..  }) = err {
                    pending
                        .write()
                        .push_back((session_ids, RawMessage::from(data)));

                    let fut = Delay::new(Instant::now() + Duration::from_secs(CHANNEL_CAST_RETRY_DELAY))
                        .then(move |_| {
                            task_handle.notify(BROADCAST_TASK_ID);
                            Ok(())
                        });

                    return Box::new(fut);
                }
            }

            Box::new(future::ok(()))
        };

        let fut_task = cast_rx
            .map(DoCast::Message)
            .select(pending_stream)
            .for_each(do_broadcast);

        Box::new(fut_task)
    }

    pub(crate) fn recv_deliver_task(
        recv_tx: Sender<TMessage>,
        pending: Arc<RwLock<VecDeque<TMessage>>>,
        task_handle: TaskHandle,
    ) -> Box<dyn Future<Item = (), Error = ()> + Send + 'static> {
        let pending_cloned = Arc::clone(&pending);
        let mut task_handle_cloned = task_handle.clone();

        // create stream from pending
        let pending_stream =
            stream::poll_fn(move || -> Poll<Option<TMessage>, SendError<TMessage>> {
                // record task handle
                task_handle_cloned.insert(RECV_DATA_TASK_ID, task::current());

                // do poll
                Ok(pending_cloned
                    .write()
                    .pop_front()
                    .map_or(Async::NotReady, |msg| Async::Ready(Some(msg))))
            });

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
    ///
    /// # Panics
    ///
    /// Panics if a protocol instance do init more than once
    pub(crate) fn do_init(&mut self, mut control: ServiceControl) {
        info!("protocol [transmission{}]: do init", self.id);

        let proto_id = self.id;
        let recv_tx = self.recv_tx.clone();
        let pending_raw_casts = Arc::clone(&self.pending_raw_casts);
        let pending_recv_data = Arc::clone(&self.pending_recv_data);

        // Take out receiver for later broadcast task
        let cast_rx = {
            let cast_rx = self.cast_rx.lock().take();

            debug_assert!(
                cast_rx.is_some(),
                "protocol [transmission]: should init once",
            );

            cast_rx.unwrap()
        };

        let broadcast_task = Self::broadcast_task(
            proto_id,
            cast_rx,
            pending_raw_casts,
            self.pending_task_handles.clone(),
            control.clone(),
        );
        let deliver_task = Self::recv_deliver_task(
            recv_tx,
            pending_recv_data,
            self.pending_task_handles.clone(),
        );

        control
            .future_task(broadcast_task)
            .expect("fail to register broadcast task");
        control
            .future_task(deliver_task)
            .expect("fail to register recv deliver task");
    }

    pub(crate) fn do_recv(&mut self, session: &SessionContext, data: RawMessage) {
        debug!(
            "protocol [transmission]: message from session [{:?}]",
            (session.id, &session.address, &session.remote_pubkey)
        );

        if let Err(()) = <TMessage as Codec>::decode(&data).and_then(|data| {
            self.pending_recv_data.write().push_back(data);
            self.pending_task_handles.notify(RECV_DATA_TASK_ID);

            Ok(())
        }) {
            let peer_id = session.remote_pubkey.as_ref().map(PeerId::from_public_key);

            self.peer_mgr.misbehave(
                peer_id,
                session.address.clone(),
                Misbehavior::InvalidMessage,
            );
        }
    }
}

impl<TMessage, TPeerManager> ServiceProtocol for TransmissionProtocol<TMessage, TPeerManager>
where
    TMessage: Codec + Send + Sync + 'static + Debug,
    TPeerManager: PeerManager + Send + Sync + Clone + 'static,
{
    fn init(&mut self, serv_ctx: &mut ServiceContext) {
        self.do_init(serv_ctx.control().clone())
    }

    fn received(&mut self, _: &mut ServiceContext, session: &SessionContext, data: RawMessage) {
        self.do_recv(session, data);
    }
}
