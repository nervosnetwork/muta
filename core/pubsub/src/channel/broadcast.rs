use std::any::Any;
use std::boxed::Box;
use std::fmt::Debug;
use std::marker::{Send, Sync};
use std::pin::Pin;
use std::sync::Arc;

use futures::channel::mpsc::{self as mpsc, TrySendError};
use futures::stream::{Fuse, Stream, StreamExt};
use futures::task::{Poll, Waker};
use uuid::Uuid;

pub type Message = Arc<Box<dyn Any + Send + Sync>>;
pub type Event = Box<dyn BroadcastEvent + Send>;

pub trait BroadcastEvent: Debug {
    fn topic(&self) -> &str;

    fn message(&self) -> &Message;

    fn boxed(self) -> Event;
}

#[derive(Clone)]
pub struct Sender {
    uuid: Uuid,
    tx: mpsc::Sender<Message>,
}

impl Sender {
    pub fn new(uuid: Uuid, tx: mpsc::Sender<Message>) -> Self {
        Sender { uuid, tx }
    }

    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    #[inline]
    pub fn try_send(&mut self, msg: Message) -> Result<(), TrySendError<Message>> {
        self.tx.try_send(msg)
    }
}

impl PartialEq for Sender {
    fn eq(&self, other: &Sender) -> bool {
        self.uuid == other.uuid
    }
}

pub struct Receiver {
    rx: Fuse<mpsc::Receiver<Event>>,
}

impl Receiver {
    pub fn new(rx: mpsc::Receiver<Event>) -> Self {
        Receiver { rx: rx.fuse() }
    }
}

impl Stream for Receiver {
    type Item = Event;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Option<Self::Item>> {
        Stream::poll_next(Pin::new(&mut self.rx), waker)
    }
}
