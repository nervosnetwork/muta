use super::common::reachable;
use crate::{event::PeerManagerEvent, peer_manager::PeerManagerHandle};

use futures::channel::mpsc::UnboundedSender;
use log::{debug, trace, warn};
use tentacle::{
    context::{ProtocolContextMutRef, SessionContext},
    multiaddr::Multiaddr,
    secio::PeerId,
    service::SessionType,
};

use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};

pub const MAX_ADDRS: usize = 10;

/// The misbehavior to report to underlying peer storage
pub enum Misbehavior {
    /// Repeat send listen addresses
    DuplicateListenAddrs,
    /// Repeat send observed address
    DuplicateObservedAddr,
    /// Timeout reached
    Timeout,
    /// Remote peer send invalid data
    InvalidData,
    /// Send too many addresses in listen addresses
    TooManyAddresses(usize),
}

/// Misbehavior report result
pub enum MisbehaveResult {
    /// Continue to run
    Continue,
    /// Disconnect this peer
    Disconnect,
}

impl MisbehaveResult {
    pub fn is_disconnect(&self) -> bool {
        match self {
            MisbehaveResult::Disconnect => true,
            _ => false,
        }
    }
}

pub struct RemoteInfo {
    pub peer_id:       PeerId,
    pub session:       SessionContext,
    pub connected_at:  Instant,
    pub timeout:       Duration,
    pub listen_addrs:  Option<Vec<Multiaddr>>,
    pub observed_addr: Option<Multiaddr>,
}

impl RemoteInfo {
    pub fn new(peer_id: PeerId, session: SessionContext, timeout: Duration) -> RemoteInfo {
        RemoteInfo {
            peer_id,
            session,
            connected_at: Instant::now(),
            timeout,
            listen_addrs: None,
            observed_addr: None,
        }
    }
}

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

/// Identify protocol
pub struct IdentifyBehaviour {
    peer_mgr:      PeerManagerHandle,
    addr_reporter: AddrReporter,
}

impl IdentifyBehaviour {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        let addr_reporter = AddrReporter::new(event_tx);

        IdentifyBehaviour {
            peer_mgr,
            addr_reporter,
        }
    }

    pub fn identify(&mut self) -> &str {
        "Identify message"
    }

    pub fn process_listens(
        &mut self,
        info: &mut RemoteInfo,
        listens: Vec<Multiaddr>,
    ) -> MisbehaveResult {
        if info.listen_addrs.is_some() {
            debug!("remote({:?}) repeat send observed address", info.peer_id);
            self.misbehave(&info.peer_id, Misbehavior::DuplicateListenAddrs)
        } else if listens.len() > MAX_ADDRS {
            self.misbehave(&info.peer_id, Misbehavior::TooManyAddresses(listens.len()))
        } else {
            trace!("received listen addresses: {:?}", listens);
            let reachable_addrs = listens.into_iter().filter(reachable).collect::<Vec<_>>();

            info.listen_addrs = Some(reachable_addrs.clone());
            self.add_remote_listen_addrs(&info.peer_id, reachable_addrs);

            MisbehaveResult::Continue
        }
    }

    pub fn process_observed(
        &mut self,
        info: &mut RemoteInfo,
        observed: Option<Multiaddr>,
    ) -> MisbehaveResult {
        if info.observed_addr.is_some() {
            debug!("remote({:?}) repeat send listen addresses", info.peer_id);
            self.misbehave(&info.peer_id, Misbehavior::DuplicateObservedAddr)
        } else {
            let observed = match observed {
                Some(addr) => addr,
                None => {
                    warn!("observed is none from peer {:?}", info.peer_id);
                    return MisbehaveResult::Disconnect;
                }
            };

            trace!("received observed address: {}", observed);
            let mut unobservable = |info: &mut RemoteInfo, observed| -> bool {
                self.add_observed_addr(&info.peer_id, observed, info.session.ty)
                    .is_disconnect()
            };

            if reachable(&observed) && unobservable(info, observed.clone()) {
                return MisbehaveResult::Disconnect;
            }

            info.observed_addr = Some(observed);
            MisbehaveResult::Continue
        }
    }

    pub fn received_identify(
        &mut self,
        _context: &mut ProtocolContextMutRef,
        _identify: &[u8],
    ) -> MisbehaveResult {
        MisbehaveResult::Continue
    }

    pub fn local_listen_addrs(&self) -> Vec<Multiaddr> {
        self.peer_mgr.listen_addrs()
    }

    pub fn add_remote_listen_addrs(&mut self, peer_id: &PeerId, addrs: Vec<Multiaddr>) {
        debug!("add remote listen {:?} addrs {:?}", peer_id, addrs);

        let identified_addrs = PeerManagerEvent::IdentifiedAddrs {
            pid: peer_id.to_owned(),
            addrs,
        };
        self.addr_reporter.report(identified_addrs);
    }

    pub fn add_observed_addr(
        &mut self,
        peer: &PeerId,
        addr: Multiaddr,
        ty: SessionType,
    ) -> MisbehaveResult {
        debug!("add observed: {:?}, addr {:?}, ty: {:?}", peer, addr, ty);

        // Noop right now
        MisbehaveResult::Continue
    }

    /// Report misbehavior
    pub fn misbehave(&mut self, peer: &PeerId, kind: Misbehavior) -> MisbehaveResult {
        match kind {
            Misbehavior::DuplicateListenAddrs => {
                debug!("peer {:?} misbehave: duplicatelisten addrs", peer)
            }
            Misbehavior::DuplicateObservedAddr => {
                debug!("peer {:?} misbehave: duplicate observed addr", peer)
            }
            Misbehavior::TooManyAddresses(size) => {
                debug!("peer {:?} misbehave: too many address {}", peer, size)
            }
            Misbehavior::InvalidData => debug!("peer {:?} misbehave: invalid data", peer),
            Misbehavior::Timeout => debug!("peer {:?} misbehave: timeout", peer),
        }

        MisbehaveResult::Disconnect
    }
}
