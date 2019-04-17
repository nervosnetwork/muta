use std::any::Any;
use std::fmt::Debug;

use futures::channel::mpsc;
use log::error;
use uuid::Uuid;

use crate::broadcast::Action;
use crate::channel::pubsub;

/// Pub/Sub Register
#[derive(Clone, Debug)]
pub struct Register {
    pub(crate) buffer: usize,

    pub(crate) act_tx: mpsc::Sender<Action>,
}

impl Register {
    pub(crate) fn new(buffer: usize, act_tx: mpsc::Sender<Action>) -> Self {
        Register { buffer, act_tx }
    }

    /// PubSub channel buffer
    pub fn buffer_size(&self) -> usize {
        self.buffer
    }

    /// Publish given message under given topic
    pub fn publish<TMessage: Any + Send>(
        &mut self,
        topic: String,
    ) -> Result<pubsub::Sender<TMessage>, ()> {
        let cloned_topic = topic.clone();
        let (tx, rx) = mpsc::channel(self.buffer);

        self.act_tx
            .try_send(Action::NewPub { topic, rx })
            .map_err(|e| log_error("publish failure", e))?;

        Ok(pubsub::Sender::new(cloned_topic, tx))
    }

    /// Subscribe given topic
    ///
    /// note: if given message type doesn't match that used in given topic, None
    /// is returned.
    pub fn subscribe<TMessage: Any + Send>(
        &mut self,
        topic: String,
    ) -> Result<pubsub::Receiver<TMessage>, ()> {
        let (tx, rx) = mpsc::channel(self.buffer);
        let uuid = Uuid::new_v4();

        self.act_tx
            .try_send(Action::NewSub { topic, uuid, tx })
            .map_err(|e| log_error("subscribe failure", e))?;

        Ok(pubsub::Receiver::new(uuid, rx))
    }

    /// Remove given topic
    pub fn unpublish(&mut self, topic: String) -> Result<(), ()> {
        self.act_tx
            .try_send(Action::RemovePub { topic })
            .map_err(|e| log_error("unpublish failure", e))
    }

    /// Unsubscribe from given topic
    pub fn unsubscribe(&mut self, topic: String, uuid: Uuid) -> Result<(), ()> {
        self.act_tx
            .try_send(Action::RemoveSub { topic, uuid })
            .map_err(|e| log_error("unsubscribe failure", e))
    }
}

fn log_error<E: Debug>(prefix: &str, e: E) {
    error!("{}: {:?}", prefix, e);
}
