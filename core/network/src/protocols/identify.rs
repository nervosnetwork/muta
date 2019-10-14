use tentacle::{
    builder::MetaBuilder,
    service::{ProtocolHandle, ProtocolMeta},
    ProtocolId,
};

use tentacle_identify::{Callback, IdentifyProtocol};

pub const NAME: &str = "chain_identify";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub struct Identify<C> {
    inner: IdentifyProtocol<C>,
}

impl<C: Callback + Send + 'static> Identify<C> {
    pub fn new(callback: C) -> Self {
        let inner = IdentifyProtocol::new(callback);

        #[cfg(feature = "allow_global_ip")]
        log::info!("network: allow global ip");

        #[cfg(feature = "allow_global_ip")]
        let inner = inner.global_ip_only(false);

        Identify { inner }
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
