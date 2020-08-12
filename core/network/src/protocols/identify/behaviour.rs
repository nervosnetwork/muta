use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::channel::mpsc::UnboundedSender;
use tentacle::multiaddr::Multiaddr;
use tentacle::secio::PeerId;
use tentacle::service::SessionType;

use crate::event::PeerManagerEvent;
use crate::peer_manager::PeerManagerHandle;

use super::common::reachable;
use super::message;
use super::protocol::StateContext;

#[derive(Clone)]
struct AddrReporter {
    inner:    UnboundedSender<PeerManagerEvent>,
    shutdown: Arc<AtomicBool>,
}

impl AddrReporter {
    pub fn new(reporter: UnboundedSender<PeerManagerEvent>) -> Self {
        AddrReporter {
            inner:    reporter,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    // TODO: upstream heart-beat check
    pub fn report(&self, event: PeerManagerEvent) {
        if self.shutdown.load(Ordering::SeqCst) {
            return;
        }

        if self.inner.unbounded_send(event).is_err() {
            log::debug!("network: discovery: peer manager offline");

            self.shutdown.store(true, Ordering::SeqCst);
        }
    }
}

/// Identify protocol
pub struct IdentifyBehaviour {
    peer_mgr:      PeerManagerHandle,
    addr_reporter: AddrReporter,
}

// Allow dead code for cfg(test)
#[allow(dead_code)]
impl IdentifyBehaviour {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        let addr_reporter = AddrReporter::new(event_tx);

        IdentifyBehaviour {
            peer_mgr,
            addr_reporter,
        }
    }

    pub fn chain_id(&self) -> String {
        self.peer_mgr.chain_id().as_ref().as_hex()
    }

    pub fn local_listen_addrs(&self) -> Vec<Multiaddr> {
        let addrs = self.peer_mgr.listen_addrs();
        let reachable_addrs = addrs.into_iter().filter(reachable);

        reachable_addrs.take(message::MAX_LISTEN_ADDRS).collect()
    }

    pub fn send_identity(&self, context: &StateContext) {
        let address_info = {
            let listen_addrs = self.local_listen_addrs();
            let observed_addr = context.observed_addr();
            message::AddressInfo::new(listen_addrs, observed_addr)
        };

        let identity = {
            let msg = message::Identity::new(self.chain_id(), address_info);
            match msg.into_bytes() {
                Ok(msg) => msg,
                Err(err) => {
                    log::warn!("encode identity msg failed {}", err);
                    context.disconnect();
                    return;
                }
            }
        };

        context.send_message(identity);
    }

    pub fn send_ack(&self, context: &StateContext) {
        let address_info = {
            let listen_addrs = self.local_listen_addrs();
            let observed_addr = context.observed_addr();
            message::AddressInfo::new(listen_addrs, observed_addr)
        };

        let acknowledge = {
            let msg = message::Acknowledge::new(address_info);
            match msg.into_bytes() {
                Ok(msg) => msg,
                Err(err) => {
                    log::warn!("encode acknowledge msg failed {}", err);
                    context.disconnect();
                    return;
                }
            }
        };

        context.send_message(acknowledge);
    }

    pub fn verify_remote_identity(
        &self,
        identity: &message::Identity,
    ) -> Result<(), super::protocol::Error> {
        if identity.chain_id != self.chain_id() {
            Err(super::protocol::Error::WrongChainId)
        } else {
            Ok(())
        }
    }

    pub fn process_listens(&self, context: &StateContext, listens: Vec<Multiaddr>) {
        let peer_id = &context.remote_peer.id;
        log::debug!("listen addresses: {:?}", listens);

        let reachable_addrs = listens.into_iter().filter(reachable).collect::<Vec<_>>();
        let identified_addrs = PeerManagerEvent::IdentifiedAddrs {
            pid:   peer_id.to_owned(),
            addrs: reachable_addrs,
        };
        self.addr_reporter.report(identified_addrs);
    }

    pub fn process_observed(&self, context: &StateContext, observed: Multiaddr) {
        let peer_id = &context.remote_peer.id;
        let session_type = context.session_context.ty;
        log::debug!("observed addr {:?} from {}", observed, context.remote_peer);

        let unobservable = |observed| -> bool {
            self.add_observed_addr(peer_id, observed, session_type)
                .is_err()
        };

        if reachable(&observed) && unobservable(observed.clone()) {
            log::warn!("unobservable {} from {}", observed, context.remote_peer);
            context.disconnect();
        }
    }

    pub fn add_observed_addr(
        &self,
        peer: &PeerId,
        addr: Multiaddr,
        ty: SessionType,
    ) -> Result<(), ()> {
        log::debug!("add observed: {:?}, addr {:?}, ty: {:?}", peer, addr, ty);

        // Noop right now
        Ok(())
    }
}
