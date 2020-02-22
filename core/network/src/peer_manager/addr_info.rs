use crate::{error::ErrorKind, traits::MultiaddrExt};

use std::{
    borrow::Borrow,
    convert::TryFrom,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tentacle::multiaddr::Multiaddr;

pub const ADDR_BACKOFF_BASE: u64 = 2;
pub const MAX_ADDR_RETRY: u8 = 8;
pub const ADDR_TIMEOUT: u64 = 10;

// Note: AddrInfo enforces peer id in multiaddr
#[derive(Debug, Clone)]
pub struct AddrInfo {
    addr:         Arc<Multiaddr>,
    connecting:   Arc<AtomicBool>,
    retry:        Arc<AtomicU8>,
    attempt_at:   Arc<AtomicU64>,
    next_attempt: Arc<AtomicU64>,
}

impl AddrInfo {
    pub fn is_connecting(&self) -> bool {
        self.connecting.load(Ordering::SeqCst)
    }

    pub fn mark_connecting(&self) {
        self.connecting.store(true, Ordering::SeqCst);
        self.attempt_at.store(Self::now(), Ordering::SeqCst);
    }

    pub fn inc_retry(&self) {
        self.connecting.store(false, Ordering::SeqCst);
        let retry = self.retry.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        debug_assert!(retry <= MAX_ADDR_RETRY + 1);

        let secs = ADDR_BACKOFF_BASE.pow(retry as u32);
        let next_attempt = Self::now().saturating_add(secs);
        self.next_attempt.store(next_attempt, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn retry(&self) -> u8 {
        self.retry.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub(crate) fn inc_retry_by(&self, n: u8) {
        self.retry.fetch_add(n, Ordering::SeqCst);
    }

    pub fn retry_ready(&self) -> bool {
        let next_attempt = Duration::from_secs(self.next_attempt.load(Ordering::SeqCst));

        Self::now() > next_attempt.as_secs()
    }

    pub fn is_timeout(&self) -> bool {
        let attempt_at = UNIX_EPOCH + Duration::from_secs(self.attempt_at.load(Ordering::SeqCst));

        self.is_connecting()
            && duration_since(SystemTime::now(), attempt_at).as_secs() > ADDR_TIMEOUT
    }

    pub fn run_out_retry(&self) -> bool {
        self.retry.load(Ordering::SeqCst) > MAX_ADDR_RETRY
    }

    fn now() -> u64 {
        duration_since(SystemTime::now(), UNIX_EPOCH).as_secs()
    }
}

impl Into<Multiaddr> for AddrInfo {
    fn into(self) -> Multiaddr {
        self.addr.as_ref().to_owned()
    }
}

impl TryFrom<Multiaddr> for AddrInfo {
    type Error = ErrorKind;

    fn try_from(ma: Multiaddr) -> Result<AddrInfo, Self::Error> {
        if !ma.has_id() {
            Err(ErrorKind::NoPeerIdMultiaddr(ma))
        } else {
            let ai = AddrInfo {
                addr:         Arc::new(ma),
                connecting:   Arc::new(AtomicBool::new(false)),
                retry:        Arc::new(AtomicU8::new(0)),
                attempt_at:   Arc::new(AtomicU64::new(0)),
                next_attempt: Arc::new(AtomicU64::new(0)),
            };

            Ok(ai)
        }
    }
}

impl Borrow<Multiaddr> for AddrInfo {
    fn borrow(&self) -> &Multiaddr {
        &self.addr
    }
}

impl PartialEq for AddrInfo {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}

impl Eq for AddrInfo {}

impl Hash for AddrInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.addr.hash(state)
    }
}

impl Deref for AddrInfo {
    type Target = Multiaddr;

    fn deref(&self) -> &Self::Target {
        &self.addr
    }
}

fn duration_since(now: SystemTime, early: SystemTime) -> Duration {
    match now.duration_since(early) {
        Ok(duration) => duration,
        Err(e) => e.duration(),
    }
}
