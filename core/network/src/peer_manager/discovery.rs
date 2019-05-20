use std::iter::FromIterator;

use hashbrown::HashSet;
use log::warn;
use rand::seq::SliceRandom;
use tentacle::{multiaddr::Multiaddr, SessionId};

use crate::p2p::protocol::discovery::{AddressManager, MisbehaveResult, Misbehavior};
use crate::peer_manager::{DefaultPeerManager, PeerManager, Source};

impl AddressManager for DefaultPeerManager {
    fn add_new_addr(&mut self, _: SessionId, addr: Multiaddr) {
        self.add_addrs(vec![addr])
    }

    fn add_new_addrs(&mut self, _: SessionId, addrs: Vec<Multiaddr>) {
        self.add_addrs(addrs)
    }

    // FIXME: implement score system
    fn misbehave(&mut self, session_id: SessionId, _kind: Misbehavior) -> MisbehaveResult {
        warn!("protocol [discovery]: misbehave: [session {}]", session_id);
        MisbehaveResult::Disconnect
    }

    fn get_random(&mut self, n: usize) -> Vec<Multiaddr> {
        let bootstrap = self.addrs(Source::BootStrap);
        let connected = self.addrs(Source::Connected);
        let disconnected = self.addrs(Source::Pool);

        let mut addrs: HashSet<Multiaddr> = HashSet::from_iter(bootstrap);
        addrs.extend(connected);
        addrs.extend(disconnected);

        let mut rng = rand::thread_rng();
        let mut addrs = addrs.into_iter().collect::<Vec<_>>();

        addrs.shuffle(&mut rng);
        addrs.into_iter().take(n).collect::<Vec<_>>()
    }
}
