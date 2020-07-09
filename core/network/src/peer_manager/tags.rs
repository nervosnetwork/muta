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
    pub fn banned_until(&self) -> Option<u64> {
        let opt_banned = { self.0.read().get(&PeerTag::ban_key()).cloned() };

        if let Some(PeerTag::Ban { expired_at }) = opt_banned {
            Some(expired_at)
        } else {
            None
        }
    }

    pub fn ban(&self, timeout: Duration) {
        if self.contains(&PeerTag::Consensus) || self.contains(&PeerTag::kk)
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
