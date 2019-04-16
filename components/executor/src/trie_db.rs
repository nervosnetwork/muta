use std::sync::Arc;

use futures::future::Future;

use core_runtime::{DataCategory, Database, DatabaseError};

#[derive(Debug)]
pub struct TrieDB<DB>
where
    DB: Database,
{
    db: Arc<DB>,
}

impl<DB> TrieDB<DB>
where
    DB: Database,
{
    pub fn new(db: Arc<DB>) -> Self {
        TrieDB { db }
    }
}

/// "TrieDB" provides state read/write capabilities for executor.
impl<DB> cita_trie::db::DB for TrieDB<DB>
where
    DB: Database,
{
    type Error = DatabaseError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        self.db.get(DataCategory::State, key).wait()
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<(), Self::Error> {
        self.db
            .insert(DataCategory::State, key.to_vec(), value.to_vec())
            .wait()
    }

    fn contains(&self, key: &[u8]) -> Result<bool, Self::Error> {
        self.db.contains(DataCategory::State, key).wait()
    }

    fn remove(&mut self, key: &[u8]) -> Result<(), Self::Error> {
        self.db.remove(DataCategory::State, key).wait()
    }
}

impl<DB> Clone for TrieDB<DB>
where
    DB: Database,
{
    fn clone(&self) -> Self {
        TrieDB {
            db: Arc::clone(&self.db),
        }
    }
}
