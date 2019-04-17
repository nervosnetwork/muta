use futures::channel::{mpsc, oneshot};
use futures::prelude::FutureExt;

use crate::broadcast::{Broadcast, PendingActions, Publishers, Subscribers};
use crate::pubsub::PubSub;
use crate::register::Register;
use crate::worker::Worker;

/// Default channel buffer size
const PUBSUB_BUFFER: usize = 65535;

/// PubSub builder
#[derive(Debug)]
pub struct Builder {
    buffer: usize,
}

impl Builder {
    /// Create a `PubSub Builder` instance with `PUBSUB_BUFFER` size
    pub fn new() -> Self {
        Builder {
            buffer: PUBSUB_BUFFER,
        }
    }

    /// Change buffer size
    pub fn buffer(mut self, buffer: usize) -> Self {
        self.buffer = buffer;
        self
    }

    /// Build `PubSub` and start background broadcast worker
    pub fn build(self) -> PubSub {
        let pubs = Publishers::new();
        let subs = Subscribers::new();
        let pending_acts = PendingActions::new();
        let (act_tx, act_rx) = mpsc::channel(self.buffer);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let broadcast_task = Broadcast::broadcast(pubs, subs, pending_acts, act_rx, shutdown_rx);
        let broadcast_worker = Worker::new(broadcast_task.shared(), shutdown_tx);

        let register = Register::new(self.buffer, act_tx);

        PubSub {
            register,
            broadcast_worker,
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}
