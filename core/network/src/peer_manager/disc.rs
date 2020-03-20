use futures::channel::mpsc::UnboundedSender;
use log::{error, warn};
use tentacle::{multiaddr::Multiaddr, SessionId};
use tentacle_discovery::{AddressManager, MisbehaveResult, Misbehavior};

use crate::{
    event::{MisbehaviorKind, PeerManagerEvent},
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
            error!("network: discovery: peer manager offline");

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
        let add_addr = PeerManagerEvent::DiscoverMultiAddrs { addrs: vec![addr] };

        self.reporter.report(add_addr);
    }

    fn add_new_addrs(&mut self, _sid: SessionId, addrs: Vec<Multiaddr>) {
        let add_multi_addrs = PeerManagerEvent::DiscoverMultiAddrs { addrs };

        self.reporter.report(add_multi_addrs);
    }

    // TODO: reduce peer score based on kind
    fn misbehave(&mut self, sid: SessionId, _kind: Misbehavior) -> MisbehaveResult {
        warn!("network: session {} misbehave", sid);

        let pid = match self.peer_mgr.peer_id(sid) {
            Some(id) => id,
            None => {
                error!("network: session {} peer id not found", sid);
                return MisbehaveResult::Disconnect;
            }
        };

        // Right now, we just remove peer
        let kind = MisbehaviorKind::Discovery;
        let peer_misbehave = PeerManagerEvent::Misbehave { pid, kind };

        self.reporter.report(peer_misbehave);
        MisbehaveResult::Disconnect
    }

    fn get_random(&mut self, n: usize) -> Vec<Multiaddr> {
        self.peer_mgr.random_addrs(n).into_iter().collect()
    }
}
