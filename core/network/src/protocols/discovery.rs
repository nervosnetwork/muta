use std::time::Duration;

use tentacle::{
    builder::MetaBuilder,
    service::{ProtocolHandle, ProtocolMeta},
    ProtocolId,
};
use tentacle_discovery::AddressManager;

pub const NAME: &str = "chain_discovery";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub struct Discovery<M> {
    inner: tentacle_discovery::DiscoveryProtocol<M>,
}

impl<M: AddressManager + Send + 'static> Discovery<M> {
    pub fn new(addr_mgr: M, sync_interval: Duration) -> Self {
        let inner_discovery = tentacle_discovery::Discovery::new(addr_mgr, Some(sync_interval));
        let inner = tentacle_discovery::DiscoveryProtocol::new(inner_discovery);

        Discovery { inner }
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || ProtocolHandle::Callback(Box::new(self.inner)))
            .build()
    }
}
