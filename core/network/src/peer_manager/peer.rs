use std::{
    collections::HashSet,
    default::Default,
    iter::FromIterator,
    sync::Arc,
    time::{Duration, Instant},
};

use protocol::types::UserAddress;
use serde_derive::{Deserialize, Serialize};
use tentacle::{
    bytes::Bytes,
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
};

pub const BACKOFF_BASE: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PeerState {
    addr_set: HashSet<Multiaddr>,
    #[serde(skip)]
    retry_count: u32,
    #[serde(skip, default = "Instant::now")]
    next_retry: Instant,
}

#[derive(Debug, Clone)]
pub struct Peer {
    id:        Arc<PeerId>,
    user_addr: Arc<UserAddress>,
    pubkey:    Arc<PublicKey>,
    state:     PeerState,
}

impl PeerState {
    pub fn new() -> Self {
        PeerState {
            addr_set:    Default::default(),
            retry_count: 0,
            next_retry:  Instant::now(),
        }
    }

    pub fn from_addrs(addrs: Vec<Multiaddr>) -> Self {
        PeerState {
            addr_set:    HashSet::from_iter(addrs),
            retry_count: 0,
            next_retry:  Instant::now(),
        }
    }

    pub(super) fn addrs(&self) -> Vec<&Multiaddr> {
        self.addr_set.iter().collect()
    }
}

impl Peer {
    pub fn new(pid: PeerId, pubkey: PublicKey) -> Self {
        Peer {
            id:        Arc::new(pid),
            user_addr: Arc::new(Peer::pubkey_to_addr(&pubkey)),
            pubkey:    Arc::new(pubkey),
            state:     PeerState::new(),
        }
    }

    pub fn from_pair(pk_addr: (PublicKey, Multiaddr)) -> Self {
        Peer {
            id:        Arc::new(pk_addr.0.peer_id()),
            user_addr: Arc::new(Peer::pubkey_to_addr(&pk_addr.0)),
            pubkey:    Arc::new(pk_addr.0),
            state:     PeerState::from_addrs(vec![pk_addr.1]),
        }
    }

    pub fn id(&self) -> &PeerId {
        &self.id
    }

    pub fn pubkey(&self) -> &PublicKey {
        &self.pubkey
    }

    pub fn user_addr(&self) -> &UserAddress {
        &self.user_addr
    }

    pub(super) fn state(&self) -> &PeerState {
        &self.state
    }

    pub fn addrs(&self) -> Vec<&Multiaddr> {
        let addr_set = &self.state.addr_set;

        addr_set.iter().collect()
    }

    pub fn owned_addrs(&self) -> Vec<Multiaddr> {
        let addr_set = &self.state.addr_set;

        addr_set.iter().map(Multiaddr::clone).collect()
    }

    pub(super) fn set_state(&mut self, state: PeerState) {
        self.state = state
    }

    pub fn add_addr(&mut self, addr: Multiaddr) {
        self.state.addr_set.insert(addr);
    }

    pub fn remove_addr(&mut self, addr: &Multiaddr) {
        self.state.addr_set.remove(addr);
    }

    pub fn retry_ready(&self) -> bool {
        Instant::now() > self.state.next_retry
    }

    pub fn retry_count(&self) -> usize {
        self.state.retry_count as usize
    }

    pub fn increase_retry(&mut self) {
        self.state.retry_count += 1;

        let secs = BACKOFF_BASE.pow(self.state.retry_count) as u64;
        self.state.next_retry = Instant::now() + Duration::from_secs(secs);
    }

    pub fn reset_retry(&mut self) {
        self.state.retry_count = 0
    }

    // # Panic
    pub(super) fn pubkey_to_addr(pubkey: &PublicKey) -> UserAddress {
        let pubkey_bytes = Bytes::from(pubkey.inner_ref().clone());

        UserAddress::from_pubkey_bytes(pubkey_bytes)
            .expect("convert from secp256k1 public key should always success")
    }
}
