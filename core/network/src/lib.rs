#![feature(async_await)]

pub mod callback_map;
pub mod common;
pub mod config;
pub mod context;
pub mod error;
pub mod inbound;
pub mod outbound;
pub mod p2p;
pub mod peer_manager;
pub mod service;

pub use config::Config;
pub use error::Error;
pub use inbound::{InboundHandle, Reactors};
pub use outbound::OutboundHandle;
pub use service::{PartialService, Service};

pub type DefaultOutboundHandle = OutboundHandle<p2p::Outbound, callback_map::Callback>;
