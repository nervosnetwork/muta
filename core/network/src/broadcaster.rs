use core_context::Context;

use crate::{message::Message, p2p};

#[derive(Clone)]
pub struct Broadcaster {
    inner: p2p::Broadcaster,
}

impl Broadcaster {
    pub fn new(p2p_broadcaster: p2p::Broadcaster) -> Self {
        Broadcaster {
            inner: p2p_broadcaster,
        }
    }

    pub fn send(&mut self, ctx: Context, msg: Message) {
        self.inner.send(ctx, p2p::Message::from(msg))
    }
}
