use crate::{event::PeerManagerEvent, peer_manager::PeerManagerHandle};

use futures::channel::mpsc::UnboundedSender;
use log::{debug, trace};
use tentacle::{
    context::{ProtocolContextMutRef, SessionContext},
    multiaddr::Multiaddr,
    secio::PeerId,
    service::SessionType,
    utils::{is_reachable, multiaddr_to_socketaddr},
    SessionId,
};

use std::{
    collections::HashMap,
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
    pub fn new(session: SessionContext, timeout: Duration) -> RemoteInfo {
        let peer_id = session
            .remote_pubkey
            .as_ref()
            .map(|key| PeerId::from_public_key(&key))
            .expect("secio must enabled!");
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

pub struct IdentifyCallback {
    peer_mgr: PeerManagerHandle,
    reporter: AddrReporter,
}

impl IdentifyCallback {
    fn new(peer_mgr: PeerManagerHandle, reporter: AddrReporter) -> Self {
        IdentifyCallback { peer_mgr, reporter }
    }

    pub fn received_identify(
        &mut self,
        _context: &mut ProtocolContextMutRef,
        _identify: &[u8],
    ) -> MisbehaveResult {
        MisbehaveResult::Continue
    }

    pub fn local_listen_addrs(&mut self) -> Vec<Multiaddr> {
        self.peer_mgr.listen_addrs()
    }

    pub fn add_remote_listen_addrs(&mut self, peer: &PeerId, addrs: Vec<Multiaddr>) {
        debug!(
            "network: add remote listen address {:?} addrs {:?}",
            peer, addrs
        );

        let pid = peer.clone();
        let identified_addrs = PeerManagerEvent::IdentifiedAddrs { pid, addrs };

        self.reporter.report(identified_addrs);
    }

    pub fn add_observed_addr(
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
    pub fn misbehave(&mut self, peer: &PeerId, kind: Misbehavior) -> MisbehaveResult {
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

/// Identify protocol
pub struct IdentifyBehaviour {
    pub(crate) callback:       IdentifyCallback,
    pub(crate) remote_infos:   HashMap<SessionId, RemoteInfo>,
    pub(crate) global_ip_only: bool,
}

impl IdentifyBehaviour {
    pub fn new(peer_mgr: PeerManagerHandle, event_tx: UnboundedSender<PeerManagerEvent>) -> Self {
        let reporter = AddrReporter::new(event_tx);
        let callback = IdentifyCallback::new(peer_mgr, reporter);

        IdentifyBehaviour {
            callback,
            remote_infos: HashMap::default(),
            global_ip_only: true,
        }
    }

    pub fn identify(&mut self) -> &[u8] {
        b"Identify message"
    }

    /// Turning off global ip only mode will allow any ip to be broadcast,
    /// default is true
    #[cfg(feature = "allow_global_ip")]
    pub fn global_ip_only(mut self, global_ip_only: bool) -> Self {
        self.global_ip_only = global_ip_only;
        self
    }

    pub fn process_listens(
        &mut self,
        context: &mut ProtocolContextMutRef,
        listens: Vec<Multiaddr>,
    ) -> MisbehaveResult {
        let session = context.session;
        let info = self
            .remote_infos
            .get_mut(&session.id)
            .expect("RemoteInfo must exists");

        if info.listen_addrs.is_some() {
            debug!("remote({:?}) repeat send observed address", info.peer_id);
            self.callback
                .misbehave(&info.peer_id, Misbehavior::DuplicateListenAddrs)
        } else if listens.len() > MAX_ADDRS {
            self.callback
                .misbehave(&info.peer_id, Misbehavior::TooManyAddresses(listens.len()))
        } else {
            trace!("received listen addresses: {:?}", listens);
            let global_ip_only = self.global_ip_only;
            let reachable_addrs = listens
                .into_iter()
                .filter(|addr| {
                    multiaddr_to_socketaddr(addr)
                        .map(|socket_addr| !global_ip_only || is_reachable(socket_addr.ip()))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            self.callback
                .add_remote_listen_addrs(&info.peer_id, reachable_addrs.clone());
            info.listen_addrs = Some(reachable_addrs);
            MisbehaveResult::Continue
        }
    }

    pub fn process_observed(
        &mut self,
        context: &mut ProtocolContextMutRef,
        observed: Multiaddr,
    ) -> MisbehaveResult {
        let session = context.session;
        let mut info = self
            .remote_infos
            .get_mut(&session.id)
            .expect("RemoteInfo must exists");

        if info.observed_addr.is_some() {
            debug!("remote({:?}) repeat send listen addresses", info.peer_id);
            self.callback
                .misbehave(&info.peer_id, Misbehavior::DuplicateObservedAddr)
        } else {
            trace!("received observed address: {}", observed);

            let global_ip_only = self.global_ip_only;
            if multiaddr_to_socketaddr(&observed)
                .map(|socket_addr| socket_addr.ip())
                .filter(|ip_addr| !global_ip_only || is_reachable(*ip_addr))
                .is_some()
                && self
                    .callback
                    .add_observed_addr(&info.peer_id, observed.clone(), info.session.ty)
                    .is_disconnect()
            {
                return MisbehaveResult::Disconnect;
            }
            info.observed_addr = Some(observed);
            MisbehaveResult::Continue
        }
    }
}
