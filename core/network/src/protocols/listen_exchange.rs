use log::error;
use tentacle::{
    builder::MetaBuilder,
    bytes::Bytes,
    context::{ProtocolContext, ProtocolContextMutRef},
    multiaddr::Multiaddr,
    service::{ProtocolHandle, ProtocolMeta},
    traits::ServiceProtocol,
    ProtocolId,
};

use crate::traits::ListenExchangeManager;

pub const NAME: &str = "chain_identify";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub struct ListenExchange<E> {
    exchange: E,
}

impl<E: ListenExchangeManager + Send + 'static> ListenExchange<E> {
    pub fn new(exchange: E) -> Self {
        ListenExchange { exchange }
    }

    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(move || ProtocolHandle::Callback(Box::new(self)))
            .build()
    }
}

// Note: fail to send listen addresses means longer bootstrap phrase, peer
// may not be discovered by other peers.
impl<E: ListenExchangeManager + Send + 'static> ServiceProtocol for ListenExchange<E> {
    fn init(&mut self, _ctx: &mut ProtocolContext) {
        // Noop
    }

    fn connected(&mut self, context: ProtocolContextMutRef, _version: &str) {
        let addr = self.exchange.listen_addr();

        let bytes = match bincode::serialize(&addr) {
            Ok(bytes) => bytes,
            Err(err) => {
                error!("network: identify protocol: serialize {}", err);

                return;
            }
        };

        if let Err(err) = context.quick_send_message(bytes.into()) {
            error!("network: identify protocol: send failure {}", err);
        }
    }

    fn received(&mut self, ctx: ProtocolContextMutRef, data: Bytes) {
        // TODO: force
        let pid = ctx
            .session
            .remote_pubkey
            .as_ref()
            .expect("peer id found")
            .peer_id();

        if let Ok(addr) = bincode::deserialize::<Multiaddr>(&data) {
            self.exchange.add_remote_listen_addr(pid, addr);
        } else {
            self.exchange.misbehave(ctx.session.id)
        }
    }
}
