use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::channel::mpsc::UnboundedSender;
use log::{debug, trace, warn};
use protocol::types::Hash;
use tentacle::multiaddr::Multiaddr;
use tentacle::secio::PeerId;
use tentacle::service::SessionType;

use crate::event::PeerManagerEvent;
use crate::peer_manager::PeerManagerHandle;

use super::common::reachable;
use super::identification::Identification;
use super::protocol::ProcedureContext;

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

    pub fn identity(&self) -> String {
        self.peer_mgr.chain_id().as_ref().as_hex()
    }

    pub fn process_listens(
        &self,
        procedure_context: &ProcedureContext,
        listens: Vec<Multiaddr>,
    ) -> MisbehaveResult {
        let peer_id = &procedure_context.peer_id;
        if listens.len() > MAX_ADDRS {
            self.misbehave(peer_id, Misbehavior::TooManyAddresses(listens.len()))
        } else {
            trace!("received listen addresses: {:?}", listens);

            let reachable_addrs = listens.into_iter().filter(reachable).collect::<Vec<_>>();
            self.add_remote_listen_addrs(peer_id, reachable_addrs);

            MisbehaveResult::Continue
        }
    }

    pub fn process_observed(
        &self,
        procedure_context: &ProcedureContext,
        observed: Option<Multiaddr>,
    ) -> MisbehaveResult {
        let peer_id = &procedure_context.peer_id;
        let session_type = procedure_context.session_context.ty;
        let observed = match observed {
            Some(addr) => addr,
            None => {
                warn!("observed is none from peer {:?}", peer_id);
                return MisbehaveResult::Disconnect;
            }
        };

        trace!("received observed address: {}", observed);
        let unobservable = |observed| -> bool {
            self.add_observed_addr(peer_id, observed, session_type)
                .is_disconnect()
        };

        if reachable(&observed) && unobservable(observed.clone()) {
            MisbehaveResult::Disconnect
        } else {
            MisbehaveResult::Continue
        }
    }

    pub fn received_identity(
        &self,
        peer_id: &PeerId,
        identification: &Identification,
        identity: &str,
    ) -> MisbehaveResult {
        use super::protocol::Error::WrongIdentity;

        let hash = match Hash::from_hex(identity) {
            Ok(h) => h,
            Err(err) => {
                warn!("decode chain id from {:?} failed: {}", peer_id, err);

                identification.failed(WrongIdentity(err.to_string()));
                return MisbehaveResult::Disconnect;
            }
        };

        if &hash != self.peer_mgr.chain_id().as_ref() {
            warn!("peer {:?} from different chain", peer_id);

            identification.failed(WrongIdentity("different chain id".to_owned()));
            return MisbehaveResult::Disconnect;
        }

        identification.pass();
        MisbehaveResult::Continue
    }

    pub fn local_listen_addrs(&self) -> Vec<Multiaddr> {
        self.peer_mgr.listen_addrs()
    }

    pub fn add_remote_listen_addrs(&self, peer_id: &PeerId, addrs: Vec<Multiaddr>) {
        debug!("add remote listen {:?} addrs {:?}", peer_id, addrs);

        let identified_addrs = PeerManagerEvent::IdentifiedAddrs {
            pid: peer_id.to_owned(),
            addrs,
        };
        self.addr_reporter.report(identified_addrs);
    }

    pub fn add_observed_addr(
        &self,
        peer: &PeerId,
        addr: Multiaddr,
        ty: SessionType,
    ) -> MisbehaveResult {
        debug!("add observed: {:?}, addr {:?}, ty: {:?}", peer, addr, ty);

        // Noop right now
        MisbehaveResult::Continue
    }

    /// Report misbehavior
    pub fn misbehave(&self, peer: &PeerId, kind: Misbehavior) -> MisbehaveResult {
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
