use super::time;
use crate::error::NetworkError;

use derive_more::Display;
use parking_lot::RwLock;
use protocol::traits::PeerTag;

use std::{collections::HashSet, time::Duration};

#[derive(Debug, Display, PartialEq, Eq)]
pub enum TagError {
    #[display(fmt = "cannot ban always allowed or consensus peer")]
    AlwaysAllow,
}

impl std::error::Error for TagError {}

impl From<TagError> for NetworkError {
    fn from(err: TagError) -> NetworkError {
        NetworkError::Internal(Box::new(err))
    }
}

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

    pub fn insert_ban(&self, timeout: Duration) -> Result<(), TagError> {
        let until = Duration::from_secs(time::now()) + timeout;
        self.insert(PeerTag::ban(until.as_secs()))
    }

    #[cfg(test)]
    pub fn set_ban_until(&self, until: u64) {
        self.0.write().insert(PeerTag::ban(until));
    }

    pub fn insert(&self, tag: PeerTag) -> Result<(), TagError> {
        if let PeerTag::Ban { .. } = tag {
            if self.contains(&PeerTag::Consensus) || self.contains(&PeerTag::AlwaysAllow) {
                return Err(TagError::AlwaysAllow);
            }
        }

        self.0.write().insert(tag);
        Ok(())
    }

    pub fn remove(&self, tag: &PeerTag) {
        self.0.write().remove(&tag);
    }

    pub fn contains(&self, tag: &PeerTag) -> bool {
        self.0.read().contains(tag)
    }
}
