use super::{time, BACKOFF_BASE, MAX_RETRY_INTERVAL};

use std::sync::{
    atomic::{AtomicU64, AtomicU8, Ordering},
    Arc,
};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Retry {
    max:             u8,
    count:           Arc<AtomicU8>,
    next_attempt_at: Arc<AtomicU64>,
}

impl Retry {
    pub fn new(max: u8) -> Self {
        Retry {
            max,
            count: Arc::new(AtomicU8::new(0)),
            next_attempt_at: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn inc(&self) {
        let count = self.count.fetch_add(1, Ordering::SeqCst).saturating_add(1);

        let mut secs = BACKOFF_BASE.pow(count as u32);
        if secs > MAX_RETRY_INTERVAL {
            secs = MAX_RETRY_INTERVAL;
        }

        let at = time::now().saturating_add(secs);
        self.next_attempt_at.store(at, Ordering::SeqCst);
    }

    pub fn eta(&self) -> u64 {
        let next_attempt_at = self.next_attempt_at.load(Ordering::SeqCst);
        next_attempt_at.saturating_sub(time::now())
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::SeqCst);
    }

    pub fn ready(&self) -> bool {
        let next_attempt_at = Duration::from_secs(self.next_attempt_at.load(Ordering::SeqCst));

        time::now() > next_attempt_at.as_secs()
    }

    pub fn count(&self) -> u8 {
        self.count.load(Ordering::SeqCst)
    }

    pub fn next_attempt_at(&self) -> u64 {
        self.next_attempt_at.load(Ordering::SeqCst)
    }

    pub fn run_out(&self) -> bool {
        self.count() > self.max
    }

    // For test and save_restore
    pub(crate) fn set_next_attempt_at(&self, at: u64) {
        self.next_attempt_at.store(at, Ordering::SeqCst);
    }

    // For test and save_restore
    pub(crate) fn set(&self, n: u8) {
        self.count.store(n, Ordering::SeqCst);
    }
}
