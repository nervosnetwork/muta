use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use futures::channel::mpsc::UnboundedSender;
use log::debug;
use tentacle::{
    context::ProtocolContextMutRef, multiaddr::Multiaddr, secio::PeerId, service::SessionType,
};
use tentacle_identify::{Callback, MisbehaveResult, Misbehavior};

use crate::{event::PeerManagerEvent, peer_manager::PeerManagerHandle};

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
    pub fn report(&mut self, event: PeerManagerEvent) {
        if self.shutdown.load(Ordering::SeqCst) {
            return;
        }

        if self.inner.unbounded_send(event).is_err() {
            debug!("network: discovery: peer manager offline");

            self.shutdown.store(true, Ordering::SeqCst);
        }
    }
}

#[derive(Clone)]
pub struct IdentifyCallback {
    peer_mgr: PeerManagerHandle,
    reporter: AddrReporter,
}

impl IdentifyCallback {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        let reporter = AddrReporter::new(event_tx);

        IdentifyCallback { peer_mgr, reporter }
    }
}

// TODO: should ask remote peer to sign a random words
// and verify received signature?
impl Callback for IdentifyCallback {
    fn identify(&mut self) -> &[u8] {
        b"Identify message"
    }

    fn received_identify(
        &mut self,
        _context: &mut ProtocolContextMutRef,
        _identify: &[u8],
    ) -> MisbehaveResult {
        MisbehaveResult::Continue
    }

    fn local_listen_addrs(&mut self) -> Vec<Multiaddr> {
        self.peer_mgr.listen_addrs()
    }

    fn add_remote_listen_addrs(&mut self, peer: &PeerId, addrs: Vec<Multiaddr>) {
        debug!(
            "network: add remote listen address {:?} addrs {:?}",
            peer, addrs
        );

        let pid = peer.clone();
        let identified_addrs = PeerManagerEvent::IdentifiedAddrs { pid, addrs };

        self.reporter.report(identified_addrs);
    }

    fn add_observed_addr(
        &mut self,
        peer: &PeerId,
        addr: Multiaddr,
        ty: SessionType,
    ) -> MisbehaveResult {
        debug!(
            "network: add observed addr: {:?}, addr {:?}, ty: {:?}",
            peer, addr, ty
        );

        // Noop right now
        MisbehaveResult::Continue
    }

    /// Report misbehavior
    fn misbehave(&mut self, peer: &PeerId, kind: Misbehavior) -> MisbehaveResult {
        match kind {
            Misbehavior::DuplicateListenAddrs => {
                debug!("network: peer {:?} misbehave: duplicatelisten addrs", peer)
            }
            Misbehavior::DuplicateObservedAddr => debug!(
                "network: peer {:?} misbehave: duplicate observed addr",
                peer
            ),
            Misbehavior::TooManyAddresses(size) => debug!(
                "network: peer {:?} misbehave: too many address {}",
                peer, size
            ),
            Misbehavior::InvalidData => debug!("network: peer {:?} misbehave: invalid data", peer),
            Misbehavior::Timeout => debug!("network: peer {:?} misbehave: timeout", peer),
        }

        MisbehaveResult::Disconnect
    }
}
