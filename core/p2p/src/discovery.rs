use std::clone::Clone;

use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::{builder::MetaBuilder, ProtocolId};
use tentacle_discovery::{Discovery, DiscoveryProtocol as InnerDiscoveryProtocol};

pub use tentacle_discovery::AddressManager as PeerManager;
pub use tentacle_discovery::{MisbehaveResult, Misbehavior, RawAddr};

/// Protocol name (handshake)
pub const PROTOCOL_NAME: &str = "discovery";

/// Protocol support versions
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

/// Discovery protocol
pub struct DiscoveryProtocol {}

impl DiscoveryProtocol {
    /// Build a `DiscoveryProtocol` instance
    pub fn build<TPeerManager>(id: ProtocolId, peer_mgr: TPeerManager) -> ProtocolMeta
    where
        TPeerManager: PeerManager + 'static + Send + Clone,
    {
        MetaBuilder::default()
            .id(id)
            .name(name!(PROTOCOL_NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || {
                let discovery = Discovery::new(peer_mgr.clone(), None);
                let boxed_disc = Box::new(InnerDiscoveryProtocol::new(discovery));
                ProtocolHandle::Callback(boxed_disc)
            })
            .build()
    }
}
