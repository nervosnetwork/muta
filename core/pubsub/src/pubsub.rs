use std::any::Any;

use futures::compat::Compat;
use futures::future::FutureObj;
use uuid::Uuid;

use crate::builder::Builder;
use crate::channel::pubsub;
use crate::register::Register;
use crate::worker::{Control, Worker};

/// PubSub
#[allow(missing_debug_implementations)]
pub struct PubSub {
    pub(crate) register:         Register,
    pub(crate) broadcast_worker: Worker,
}

impl PubSub {
    /// Create a `PubSub` builder
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Return 'PubSub' channel buffer size
    pub fn buffer_size(&self) -> usize {
        self.register.buffer_size()
    }

    /// Return 'PubSub' register clone
    pub fn register(&self) -> Register {
        self.register.clone()
    }

    /// Publish given message under given topic
    pub fn publish<TMessage: Any + Send>(
        &mut self,
        topic: String,
    ) -> Result<pubsub::Sender<TMessage>, ()> {
        self.register.publish(topic)
    }

    /// Subscribe given topic
    ///
    /// note: if given message type doesn't match that used in given topic, None
    /// is returned.
    pub fn subscribe<TMessage: Any + Send>(
        &mut self,
        topic: String,
    ) -> Result<pubsub::Receiver<TMessage>, ()> {
        self.register.subscribe(topic)
    }

    /// Remove given topic
    pub fn unpublish(&mut self, topic: String) -> Result<(), ()> {
        self.register.unpublish(topic)
    }

    /// Unsubscribe from given topic
    pub fn unsubscribe(&mut self, topic: String, uuid: Uuid) -> Result<(), ()> {
        self.register.unsubscribe(topic, uuid)
    }

    /// Run pubsub in single thread
    pub fn start(self) -> Self {
        PubSub {
            register:         self.register,
            broadcast_worker: self.broadcast_worker.start_loop(),
        }
    }

    /// Return inner pubsub future task, it's control and `Register`. Use this
    /// method if you want to spawn pubsub future yourself.
    pub fn fut_task(self) -> (Compat<FutureObj<'static, ()>>, Control, Register) {
        let (futobj, ctrl) = self.broadcast_worker.task();
        let futobj = Compat::new(futobj);

        (futobj, ctrl, self.register)
    }

    /// Shutdown this pubsub instance
    pub fn shutdown(self) -> Result<(), ()> {
        self.broadcast_worker.shutdown()
    }
}
