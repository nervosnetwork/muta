use tentacle::service::{DialProtocol, ProtocolHandle, ProtocolMeta};
use tentacle::{builder::MetaBuilder, multiaddr::Multiaddr, traits::ServiceProtocol};
use tentacle::{
    context::{ProtocolContext, ServiceContext},
    ProtocolId,
};

use std::time::Duration;

/// Protocol name (handshake)
pub const PROTOCOL_NAME: &str = "connec";

/// Protocol support versions
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

/// Connect to new addresses interval (seconds)
pub const CONNEC_DIAL_INTERVAL: u64 = 5;

/// Internal notify token
pub(crate) const CONNEC_PEER_TOKEN: u64 = 77;

/// Remote multiaddr to connect
#[derive(Debug)]
pub struct RemoteAddr {
    addr:  Multiaddr,
    proto: DialProtocol,
}

impl RemoteAddr {
    /// create new `RemoteAddr`
    pub fn new(addr: Multiaddr, proto: DialProtocol) -> Self {
        RemoteAddr { addr, proto }
    }

    /// multiaddr
    pub fn addr(&self) -> &Multiaddr {
        &self.addr
    }
}

/// Peer manager for `connec` protocol
pub trait PeerManager: Clone + Send + Sync {
    /// Get unconnected multiaddrs to connect
    fn unconnected_multiaddrs(&mut self) -> Vec<RemoteAddr>;
}

/// Pure stateless interval multiaddr connection protocol
pub struct ConnecProtocol<TPeerManager> {
    id:       ProtocolId,
    peer_mgr: TPeerManager,
}

impl<TPeerManager> ConnecProtocol<TPeerManager>
where
    TPeerManager: PeerManager + 'static,
{
    /// build a `ConnecProtocol` instance
    pub fn build(id: ProtocolId, peer_mgr: TPeerManager) -> ProtocolMeta {
        MetaBuilder::default()
            .id(id)
            .name(name!(PROTOCOL_NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || {
                let boxed_proto = Box::new(ConnecProtocol {
                    id,
                    peer_mgr: peer_mgr.clone(),
                });
                ProtocolHandle::Callback(boxed_proto)
            })
            .build()
    }

    pub(crate) fn do_connec(&mut self, serv_ctx: &mut ServiceContext, token: u64) {
        if CONNEC_PEER_TOKEN == token {
            let peers = self.peer_mgr.unconnected_multiaddrs();

            for RemoteAddr { addr, proto } in peers {
                serv_ctx.dial(addr, proto);
            }
        }
    }
}

impl<TPeerManager> ServiceProtocol for ConnecProtocol<TPeerManager>
where
    TPeerManager: PeerManager + 'static,
{
    fn init(&mut self, proto_ctx: &mut ProtocolContext) {
        proto_ctx.set_service_notify(
            self.id,
            Duration::from_secs(CONNEC_DIAL_INTERVAL),
            CONNEC_PEER_TOKEN,
        )
    }

    fn notify(&mut self, proto_ctx: &mut ProtocolContext, token: u64) {
        self.do_connec(proto_ctx, token);
    }
}
