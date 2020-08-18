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
mod selfcheck;
mod service;
#[cfg(test)]
mod test;
mod traits;

pub use config::NetworkConfig;
pub use error::NetworkError;
pub use message::{serde, serde_multi};
pub use service::{NetworkService, NetworkServiceHandle};

#[cfg(feature = "diagnostic")]
pub use peer_manager::diagnostic::{DiagnosticEvent, TrustReport};

pub use tentacle::secio::PeerId;

use protocol::Bytes;
use tentacle::secio::PublicKey;

pub trait PeerIdExt {
    fn from_pubkey_bytes<'a, B: AsRef<[u8]> + 'a>(bytes: B) -> Result<PeerId, NetworkError> {
        let pubkey = PublicKey::secp256k1_raw_key(bytes.as_ref())
            .map_err(|_| NetworkError::InvalidPublicKey)?;

        Ok(PeerId::from_public_key(&pubkey))
    }

    fn from_bytes<'a, B: AsRef<[u8]> + 'a>(bytes: B) -> Result<PeerId, NetworkError> {
        PeerId::from_bytes(bytes.as_ref().to_vec()).map_err(|_| NetworkError::InvalidPeerId)
    }

    fn to_string(&self) -> String;

    fn into_bytes_ext(self) -> Bytes;

    fn from_str_ext<'a, S: AsRef<str> + 'a>(s: S) -> Result<PeerId, NetworkError> {
        s.as_ref().parse().map_err(|_| NetworkError::InvalidPeerId)
    }
}

impl PeerIdExt for PeerId {
    fn into_bytes_ext(self) -> Bytes {
        Bytes::from(self.into_bytes())
    }

    fn to_string(&self) -> String {
        self.to_base58()
    }
}
