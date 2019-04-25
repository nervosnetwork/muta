use std::any::Any;
use std::borrow::ToOwned;
use std::error::Error;
use std::fmt;
use std::marker::{PhantomData, Send};
use std::pin::Pin;
use std::sync::Arc;

use futures::channel::mpsc;
use futures::prelude::Stream;
use futures::task::{Context, Poll};
use uuid::Uuid;

use crate::channel::broadcast::{BroadcastEvent, Event, Message};

#[derive(Debug)]
struct PubEvent {
    pub topic: String,
    pub msg:   Message,
}

impl PubEvent {
    pub fn new<TMessage: Any + Send + Sync>(topic: String, msg: TMessage) -> Self {
        let msg: Message = Arc::new(Box::new(msg));

        PubEvent { topic, msg }
    }
}

impl BroadcastEvent for PubEvent {
    fn topic(&self) -> &str {
        &self.topic
    }

    fn message(&self) -> &Message {
        &self.msg
    }

    fn boxed(self) -> Event {
        Box::new(self)
    }
}

/// PubSub channel Sender
#[derive(Debug, Clone)]
pub struct Sender<TMessage>
where
    TMessage: Any + Send,
{
    topic: String,
    tx:    mpsc::Sender<Event>,

    pin_msg_type: PhantomData<TMessage>,
}

impl<TMessage> Sender<TMessage>
where
    TMessage: Any + Send,
{
    pub(crate) fn new(topic: String, tx: mpsc::Sender<Event>) -> Self {
        Sender {
            topic,
            tx,
            pin_msg_type: PhantomData,
        }
    }
}

impl<TMessage> Sender<TMessage>
where
    TMessage: Any + Send + Sync + Clone,
{
    /// Try publish given generic message to subscribers.
    #[inline]
    pub fn try_send(&mut self, msg: TMessage) -> Result<(), TrySendError<TMessage>> {
        let event = PubEvent::new(self.topic.to_owned(), msg.clone()).boxed();

        match self.tx.try_send(event) {
            Err(err) => {
                let err = err.into_send_error();
                let val = msg.clone();

                Err(TrySendError { err, val })
            }
            Ok(()) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SendErrorKind {
    Full,
    Disconnected,
}

/// Error for `try_send`
pub struct TrySendError<TMessage> {
    err: mpsc::SendError,
    val: TMessage,
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = {
            if self.err.is_full() {
                SendErrorKind::Full
            } else {
                SendErrorKind::Disconnected
            }
        };

        fmt.debug_struct("TrySendError")
            .field("kind", &kind)
            .finish()
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.err.is_full() {
            write!(fmt, "send failed because channel is full")
        } else {
            write!(fmt, "send failed because receiver is gone")
        }
    }
}

impl<T: Any> Error for TrySendError<T> {
    fn description(&self) -> &str {
        if self.err.is_full() {
            "send failed because channel is full"
        } else {
            "send failed because receiver is gone"
        }
    }
}

impl<T> TrySendError<T> {
    /// True if channel is full
    pub fn is_full(&self) -> bool {
        self.err.is_full()
    }

    /// True if channel is disconnected
    pub fn is_disconnected(&self) -> bool {
        self.err.is_disconnected()
    }

    /// Consume error and return message that fail to send
    pub fn into_inner(self) -> T {
        self.val
    }

    /// Consume error and return inner error
    pub fn into_send_error(self) -> mpsc::SendError {
        self.err
    }
}

/// PubSub channel receiver
#[derive(Debug)]
pub struct Receiver<TMessage>
where
    TMessage: Any + Send,
{
    uuid: Uuid,
    rx:   mpsc::Receiver<Message>,

    pin_msg_type: PhantomData<TMessage>,
}

impl<TMessage> Receiver<TMessage>
where
    TMessage: Any + Send,
{
    /// Create a `PubSub` channel receiver.
    ///
    /// Uuid to identify subscriber
    pub fn new(uuid: Uuid, rx: mpsc::Receiver<Message>) -> Self {
        Receiver {
            uuid,
            rx,
            pin_msg_type: PhantomData,
        }
    }

    /// Subscriber's uuid
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }
}

impl<TMessage> Stream for &mut Receiver<TMessage>
where
    TMessage: Any + Send + Clone,
{
    type Item = Option<TMessage>;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
        match Stream::poll_next(Pin::new(&mut self.rx), ctx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None), // close the stream
            Poll::Ready(Some(any_box)) => {
                let e = any_box.downcast_ref::<TMessage>().map(ToOwned::to_owned);
                // Warp in `Some` so that we will not close the stream by
                // accident.
                Poll::Ready(Some(e))
            }
        }
    }
}
