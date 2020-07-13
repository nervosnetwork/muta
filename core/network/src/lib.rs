mod common;
mod compression;
mod config;
mod connection;
mod endpoint;
mod error;
mod event;
mod message;
mod metrics;
mod outbound;
mod peer_manager;
mod protocols;
mod reactor;
mod rpc;
mod rpc_map;
mod selfcheck;
mod service;
#[cfg(test)]
mod test;
mod traits;

pub use config::NetworkConfig;
pub use message::{serde, serde_multi};
pub use service::{NetworkService, NetworkServiceHandle};

#[cfg(feature = "diagnostic")]
pub use peer_manager::diagnostic::{DiagnosticEvent, TrustReport};

pub use tentacle::secio::PeerId;
