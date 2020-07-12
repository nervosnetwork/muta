mod addr;
mod behaviour;
mod message;
#[allow(dead_code)]
mod message_mol; // Auto generated code
mod protocol;
mod substream;

use self::protocol::DiscoveryProtocol;
use addr::AddressManager;
use behaviour::DiscoveryBehaviour;

use crate::{event::PeerManagerEvent, peer_manager::PeerManagerHandle};

use futures::channel::mpsc::UnboundedSender;
use tentacle::{
    builder::MetaBuilder,
    service::{ProtocolHandle, ProtocolMeta},
    ProtocolId,
};

use std::time::Duration;

pub const NAME: &str = "chain_discovery";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub struct Discovery(DiscoveryProtocol);

impl Discovery {
    pub fn new(
        peer_mgr: PeerManagerHandle,
        event_tx: UnboundedSender<PeerManagerEvent>,
        sync_interval: Duration,
    ) -> Self {
        let address_manager = AddressManager::new(peer_mgr, event_tx);
        let behaviour = DiscoveryBehaviour::new(address_manager, Some(sync_interval));

        #[cfg(feature = "allow_global_ip")]
        log::info!("network: allow global ip");

        #[cfg(feature = "allow_global_ip")]
        let behaviour = behaviour.global_ip_only(false);

        Discovery(DiscoveryProtocol::new(behaviour))
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || ProtocolHandle::Callback(Box::new(self.0)))
            .build()
    }
}
