use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::{builder::MetaBuilder, traits::ServiceProtocol, ProtocolId};
use tentacle_discovery::{Discovery, DiscoveryProtocol as InnerDiscoveryProtocol};

pub use tentacle_discovery::AddressManager;
pub use tentacle_discovery::{MisbehaveResult, Misbehavior, RawAddr};

/// Protocol name (handshake)
pub const PROTOCOL_NAME: &str = "discovery";

/// Protocol support versions
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.2"];

/// Discovery protocol
pub struct DiscoveryProtocol;

impl DiscoveryProtocol {
    /// Return `ProtocolMeta` of discovery protocol
    pub fn meta<TManager>(id: ProtocolId, mgr: TManager) -> ProtocolMeta
    where
        TManager: AddressManager + Send + 'static,
    {
        let service_handle =
            move || -> ProtocolHandle<Box<dyn ServiceProtocol + Send + 'static>> {
                let discovery = Discovery::new(mgr, None);
                let boxed_disc = Box::new(InnerDiscoveryProtocol::new(discovery));
                ProtocolHandle::Callback(boxed_disc)
            };

        MetaBuilder::default()
            .id(id)
            .name(name!(PROTOCOL_NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(service_handle)
            .build()
    }
}
