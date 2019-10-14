use futures::channel::mpsc::UnboundedSender;
use log::debug;
use tentacle::{multiaddr::Multiaddr, SessionId};
use tentacle_discovery::{AddressManager, MisbehaveResult, Misbehavior};

use crate::{
    event::{PeerManagerEvent, RemoveKind},
    peer_manager::PeerManagerHandle,
};

struct AddrReporter {
    inner:    UnboundedSender<PeerManagerEvent>,
    shutdown: bool,
}

impl AddrReporter {
    pub fn new(reporter: UnboundedSender<PeerManagerEvent>) -> Self {
        AddrReporter {
            inner:    reporter,
            shutdown: false,
        }
    }

    // TODO: upstream heart-beat check
    pub fn report(&mut self, event: PeerManagerEvent) {
        if self.shutdown {
            return;
        }

        if self.inner.unbounded_send(event).is_err() {
            debug!("network: discovery: peer manager offline");

            self.shutdown = true;
        }
    }
}

pub struct DiscoveryAddrManager {
    peer_mgr: PeerManagerHandle,
    reporter: AddrReporter,
}

impl DiscoveryAddrManager {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        let reporter = AddrReporter::new(event_tx);

        DiscoveryAddrManager { peer_mgr, reporter }
    }
}

impl AddressManager for DiscoveryAddrManager {
    fn add_new_addr(&mut self, _sid: SessionId, addr: Multiaddr) {
        let add_addr = PeerManagerEvent::DiscoverAddr { addr };

        self.reporter.report(add_addr);
    }

    fn add_new_addrs(&mut self, _sid: SessionId, addrs: Vec<Multiaddr>) {
        let add_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs { addrs };

        self.reporter.report(add_multi_addrs);
    }

    // TODO: reduce peer score based on kind
    fn misbehave(&mut self, sid: SessionId, _kind: Misbehavior) -> MisbehaveResult {
        debug!("network: session {} misbehave", sid);

        // Right now, we just remove peer
        let kind = RemoveKind::BadSessionPeer("discovery misbehavior".to_owned());
        let remove_peer_by_session = PeerManagerEvent::RemovePeerBySession { sid, kind };

        self.reporter.report(remove_peer_by_session);
        MisbehaveResult::Disconnect
    }

    fn get_random(&mut self, n: usize) -> Vec<Multiaddr> {
        self.peer_mgr.random_addrs(n)
    }
}
