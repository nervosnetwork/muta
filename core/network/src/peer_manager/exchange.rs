use futures::channel::mpsc::UnboundedSender;
use log::debug;
use tentacle::{multiaddr::Multiaddr, secio::PeerId, SessionId};

use crate::{
    event::PeerManagerEvent, peer_manager::PeerManagerHandle, traits::ListenExchangeManager,
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

pub struct PeerListenExchange {
    peer_mgr: PeerManagerHandle,
    reporter: AddrReporter,
}

impl PeerListenExchange {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        let reporter = AddrReporter::new(event_tx);

        PeerListenExchange { peer_mgr, reporter }
    }
}

impl ListenExchangeManager for PeerListenExchange {
    fn listen_addr(&self) -> Multiaddr {
        self.peer_mgr.inner.listen().expect("no listen")
    }

    fn add_remote_listen_addr(&mut self, pid: PeerId, addr: Multiaddr) {
        let add_addr = PeerManagerEvent::AddPeerAddr { pid, addr };

        self.reporter.report(add_addr);
    }

    fn misbehave(&mut self, _sid: SessionId) {
        // TODO: reduce score
        // Noop
    }
}
