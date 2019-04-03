pub mod config;
pub mod message;
pub(crate) mod p2p;

pub use config::Config;
pub use message::Message;
pub use p2p::Broadcaster;

use p2p::{Config as P2PConfig, Message as P2PMessage, Service as P2PService};

use std::error::Error;

pub struct Network {
    p2p: P2PService,
}

impl Network {
    pub fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let p2p = P2PService::build()
            .config(P2PConfig::from_raw(config.p2p)?)
            .launch();

        Ok(Network { p2p })
    }

    pub fn send(&mut self, msg: Message) {
        self.p2p.send(P2PMessage::from(msg))
    }

    pub fn broadcaster(&self) -> Broadcaster {
        self.p2p.broadcaster()
    }

    pub fn shutdown(self) {
        self.p2p.shutdown()
    }
}
