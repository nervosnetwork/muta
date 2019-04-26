#![feature(async_await, await_macro, futures_api)]

pub mod broadcaster;
pub mod config;
pub mod message;
pub mod p2p;
pub mod reactor;

use std::error::Error;

use futures::sync::mpsc::Receiver;

use core_context::Context;

use crate::p2p::{Config as P2PConfig, Message as P2PMessage, Service as P2PService};
use crate::reactor::{outbound::OutboundMessage, Reaction, ReactorMessage};

pub use broadcaster::Broadcaster;
pub use config::Config;
pub use message::Message;
pub use reactor::Reactor;

pub struct Network {
    p2p: P2PService,
}

impl Network {
    pub fn new<R>(
        config: Config,

        outbound_rx: Receiver<OutboundMessage>,
        reactor: R,
    ) -> Result<Self, Box<dyn Error>>
    where
        R: Reactor<Input = ReactorMessage, Output = Reaction<ReactorMessage>> + Send + 'static,
    {
        let p2p = P2PService::build(reactor, outbound_rx)
            .config(P2PConfig::from_raw(config.p2p)?)
            .launch();

        Ok(Network { p2p })
    }

    pub fn send(&mut self, ctx: Context, msg: Message) {
        self.p2p.send(ctx, P2PMessage::from(msg))
    }

    pub fn broadcaster(&self) -> Broadcaster {
        Broadcaster::new(self.p2p.broadcaster())
    }

    pub async fn shutdown(self) {
        await!(self.p2p.shutdown())
    }
}
