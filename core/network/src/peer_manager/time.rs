use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    duration_since(SystemTime::now(), UNIX_EPOCH).as_secs()
}

pub fn duration_since(now: SystemTime, early: SystemTime) -> Duration {
    match now.duration_since(early) {
        Ok(duration) => duration,
        Err(e) => e.duration(),
    }
}
