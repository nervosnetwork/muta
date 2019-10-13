use std::{
    collections::HashSet,
    default::Default,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use derive_more::Display;
use protocol::types::UserAddress;
use serde_derive::{Deserialize, Serialize};
use tentacle::{
    bytes::Bytes,
    multiaddr::Multiaddr,
    secio::{PeerId, PublicKey},
};

pub const BACKOFF_BASE: usize = 5;
pub const VALID_ATTEMPT_INTERVAL: u64 = 4;

// TODO: display next_retry
#[derive(Debug, Clone, Serialize, Deserialize, Display)]
#[display(
    fmt = "addrs: {:?}, retry: {}, last_connect: {}, alive: {}",
    addr_set,
    retry_count,
    connect_at,
    alive
)]
pub(super) struct PeerState {
    // Peer listen address set
    addr_set: HashSet<Multiaddr>,

    #[serde(skip)]
    retry_count: u32,

    #[serde(skip, default = "Instant::now")]
    next_retry: Instant,

    // Connect at (timestamp)
    connect_at: u64,

    // Disconnect as (timestamp)
    disconnect_at: u64,

    // Attempt at (timestamp)
    #[serde(skip)]
    attempt_at: u64,

    // Alive (seconds)
    alive: u64,
}

#[derive(Debug, Clone, Display)]
#[display(
    fmt = "peer id: {:?}, user addr: {:?}, state: {}",
    id,
    user_addr,
    state
)]
pub struct Peer {
    id:        Arc<PeerId>,
    user_addr: Arc<UserAddress>,
    pubkey:    Arc<PublicKey>,
    state:     PeerState,
}

impl PeerState {
    pub fn new() -> Self {
        PeerState {
            addr_set:      Default::default(),
            retry_count:   0,
            next_retry:    Instant::now(),
            connect_at:    0,
            disconnect_at: 0,
            attempt_at:    0,
            alive:         0,
        }
    }

    pub fn from_addrs(addrs: Vec<Multiaddr>) -> Self {
        let mut state = PeerState::new();
        state.addr_set.extend(addrs.into_iter());

        state
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

    pub fn update_connect(&mut self) {
        self.state.connect_at = duration_since(SystemTime::now(), UNIX_EPOCH).as_secs();
    }

    pub fn update_disconnect(&mut self) {
        self.state.disconnect_at = duration_since(SystemTime::now(), UNIX_EPOCH).as_secs();
    }

    pub fn update_alive(&mut self) {
        let connect_at = UNIX_EPOCH + Duration::from_secs(self.state.connect_at);

        self.state.alive = duration_since(SystemTime::now(), connect_at).as_secs();
    }

    pub fn alive(&self) -> u64 {
        self.state.alive
    }

    pub fn retry_ready(&self) -> bool {
        Instant::now() > self.state.next_retry
    }

    pub fn retry_count(&self) -> usize {
        self.state.retry_count as usize
    }

    pub fn increase_retry(&mut self) {
        let last_attempt = UNIX_EPOCH + Duration::from_secs(self.state.attempt_at);

        // Every time we try connect to a peer, we use all addresses. If
        // fail, we should only increase once.
        if duration_since(SystemTime::now(), last_attempt).as_secs() < VALID_ATTEMPT_INTERVAL {
            return;
        }

        self.state.retry_count += 1;
        self.state.attempt_at = duration_since(SystemTime::now(), UNIX_EPOCH).as_secs();

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

fn duration_since(now: SystemTime, early: SystemTime) -> Duration {
    match now.duration_since(early) {
        Ok(duration) => duration,
        Err(e) => e.duration(),
    }
}
