use bytes::Bytes;
use futures::prelude::{Async, Future, Poll, Stream};
use futures::stream;
use futures::sync::mpsc::{channel, Receiver, Sender};
use futures::task::{self as task, Task};
use log::{debug, info, warn};
use parking_lot::{Mutex, RwLock};
use tentacle::context::{ServiceContext, SessionContext};
use tentacle::service::{ProtocolHandle, ProtocolMeta, ServiceControl, ServiceTask};
use tentacle::{
    builder::MetaBuilder, error::Error, multiaddr::Multiaddr, secio::PeerId,
    traits::ServiceProtocol, ProtocolId, SessionId,
};

use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::Send;
use std::sync::Arc;

/// Protocol name (handshake)
pub const PROTOCOL_NAME: &str = "transmission";

/// Protocol support versions
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

/// Channel buffer size
pub const CHANNEL_BUFFERS: usize = 8;

const BROADCAST_TASK_ID: usize = 1;
const RECV_DATA_TASK_ID: usize = 2;

/// Low-level transport data type
pub type RawMessage = Bytes;

/// `Message` codec
pub trait Codec: Sized {
    /// Encode `Message` type to transport data type
    fn encode(self) -> RawMessage;

    /// Decode raw bytes to `Message` type
    fn decode(raw: &[u8]) -> Result<Self, ()>;
}

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

// Wrapper around cast and recv stream task
pub(crate) struct TaskHandle {
    inner: Arc<RwLock<(Option<Task>, Option<Task>)>>,
}

impl TaskHandle {
    pub fn notify(&self, id: usize) {
        let maybe_task = match id {
            BROADCAST_TASK_ID => self.inner.read().0.clone(),
            RECV_DATA_TASK_ID => self.inner.read().1.clone(),
            _ => unreachable!(),
        };

        maybe_task.and_then(|task| {
            task.notify();
            Some(())
        });
    }

    pub fn insert(&mut self, id: usize, task: Task) {
        match id {
            BROADCAST_TASK_ID => self.inner.write().0 = Some(task),
            RECV_DATA_TASK_ID => self.inner.write().1 = Some(task),
            _ => unreachable!(),
        }
    }
}

impl Default for TaskHandle {
    fn default() -> Self {
        TaskHandle {
            inner: Arc::new(RwLock::new((None, None))),
        }
    }
}

impl Clone for TaskHandle {
    fn clone(&self) -> Self {
        TaskHandle {
            inner: Arc::clone(&self.inner),
        }
    }
}

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
        control: &mut ServiceControl,
    ) -> Box<dyn Future<Item = (), Error = ()> + Send + 'static> {
        let mut control = control.clone();

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

        let mut do_broadcast = move |session_ids: Option<Vec<SessionId>>, raw_msg: RawMessage| {
            if let Err(err) = control.filter_broadcast(session_ids, proto_id, raw_msg.to_vec()) {
                debug!("protocol [tranmission]: fail to send message");

                // move it to pending
                if let Error::TaskFull(ServiceTask::ProtocolMessage {
                    session_ids, data, ..
                }) = err
                {
                    pending
                        .write()
                        .push_back((session_ids, RawMessage::from(data)));

                    task_handle.notify(BROADCAST_TASK_ID);
                }
            }
        };

        let fut_task = cast_rx
            .map(DoCast::Message)
            .select(pending_stream)
            .for_each(move |do_cast| {
                let (session_ids, raw_msg) = unpark_cast(do_cast);
                do_broadcast(session_ids, raw_msg);
                Ok(()) // continue loop our task
            });

        Box::new(fut_task)
    }

    pub(crate) fn recv_deliver_task(
        mut recv_tx: Sender<TMessage>,
        pending: Arc<RwLock<VecDeque<TMessage>>>,
        task_handle: TaskHandle,
    ) -> Box<dyn Future<Item = (), Error = ()> + Send + 'static> {
        let pending_cloned = Arc::clone(&pending);
        let mut task_handle_cloned = task_handle.clone();

        // create stream from pending
        let pending_stream = stream::poll_fn(move || -> Poll<Option<TMessage>, ()> {
            // record task handle
            task_handle_cloned.insert(RECV_DATA_TASK_ID, task::current());

            // do poll
            Ok(pending_cloned
                .write()
                .pop_front()
                .map_or(Async::NotReady, |msg| Async::Ready(Some(msg))))
        });

        let deliver_task = pending_stream.for_each(move |recv_msg| {
            if let Err(err) = recv_tx.try_send(recv_msg) {
                warn!(
                    "protocol [transmission]: fail to deliver recv msg: [{}]",
                    err
                );

                // unpark TrySendError to recover msg, push it back to pending
                pending.write().push_front(err.into_inner());

                // notify
                task_handle.clone().notify(RECV_DATA_TASK_ID);
            }
            Ok(())
        });

        Box::new(deliver_task)
    }

    pub fn do_init(&mut self, control: &mut ServiceContext) {
        info!("protocol [transmission{}]: do init", self.id);

        let proto_id = self.id;
        let control = control.control();
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
            control,
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

    pub fn do_recv(
        &mut self,
        _control: &mut ServiceContext,
        session: &SessionContext,
        data: RawMessage,
    ) {
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
    fn init(&mut self, control: &mut ServiceContext) {
        self.do_init(control)
    }

    fn received(
        &mut self,
        control: &mut ServiceContext,
        session: &SessionContext,
        data: RawMessage,
    ) {
        self.do_recv(control, session, data);
    }
}

// Default implement for `RawMessage`
#[cfg(not(feature = "prost-message"))]
impl Codec for RawMessage {
    fn encode(self) -> RawMessage {
        self
    }

    fn decode(raw: &[u8]) -> Result<RawMessage, ()> {
        Ok(Bytes::from(raw))
    }
}

// Implement `prost` out-of-box support
#[cfg(feature = "prost-message")]
impl<TMessage: prost::Message + std::default::Default> Codec for TMessage {
    fn encode(self) -> RawMessage {
        let mut msg = vec![];

        if let Err(err) = <TMessage as prost::Message>::encode(&self, &mut msg) {
            // system should not provide non-encodeable message
            // this means fatal error, but dont panic.
            log::error!("protocol [transmission]: *! encode failure: {:?}", err);
        }

        Bytes::from(msg)
    }

    fn decode(raw: &[u8]) -> Result<TMessage, ()> {
        <TMessage as prost::Message>::decode(raw.to_owned())
            .map_err(|err| log::error!("protocol [transmission]: *! decode failure: {:?}", err))
    }
}
