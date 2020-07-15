use super::time;
use crate::error::ErrorKind;

use parking_lot::RwLock;
use protocol::traits::PeerTag;

use std::{collections::HashSet, time::Duration};

#[derive(Debug)]
pub struct Tags(RwLock<HashSet<PeerTag>>);

impl Default for Tags {
    fn default() -> Self {
        Tags(Default::default())
    }
}

impl Tags {
    pub fn get_banned_until(&self) -> Option<u64> {
        let opt_banned = { self.0.read().get(&PeerTag::ban_key()).cloned() };

        if let Some(PeerTag::Ban { until }) = opt_banned {
            Some(until)
        } else {
            None
        }
    }

    pub fn insert_ban(&self, timeout: Duration) -> Result<(), ErrorKind> {
        if self.contains(&PeerTag::Consensus) || self.contains(&PeerTag::AlwaysAllow) {
            return Err(ErrorKind::Untaggable(
                "consensus and always allow cannot be ban".to_owned(),
            ));
        }

        let until = Duration::from_secs(time::now()) + timeout;
        self.0.write().insert(PeerTag::ban(until.as_secs()));

        Ok(())
    }

    #[cfg(test)]
    pub fn set_ban_until(&self, until: u64) {
        self.0.write().insert(PeerTag::ban(until));
    }

    pub fn insert(&self, tag: PeerTag) {
        self.0.write().insert(tag);
    }

    pub fn remove(&self, tag: &PeerTag) {
        self.0.write().remove(&tag);
    }

    pub fn contains(&self, tag: &PeerTag) -> bool {
        self.0.read().contains(tag)
    }
}
