use std::sync::Arc;

use futures::executor::block_on;
use parking_lot::RwLock;
use std::collections::HashMap;

use core_context::Context;
use core_runtime::{DataCategory, Database, DatabaseError};

#[derive(Debug)]
pub struct TrieDB<DB>
where
    DB: Database,
{
    db:    Arc<DB>,
    cache: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl<DB> TrieDB<DB>
where
    DB: Database,
{
    pub fn new(db: Arc<DB>) -> Self {
        TrieDB {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// "TrieDB" provides state read/write capabilities for executor.
impl<DB> cita_trie::DB for TrieDB<DB>
where
    DB: Database,
{
    type Error = DatabaseError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        match self.cache.read().get(key) {
            Some(v) => Ok(Some(v.to_vec())),
            None => block_on(self.db.get(Context::new(), DataCategory::State, key)),
        }
    }

    fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Self::Error> {
        self.cache.write().insert(key, value);
        Ok(())
    }

    fn contains(&self, key: &[u8]) -> Result<bool, Self::Error> {
        if self.cache.read().contains_key(key) {
            Ok(true)
        } else {
            block_on(self.db.contains(Context::new(), DataCategory::State, key))
        }
    }

    fn remove(&self, _key: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        let mut cache = self.cache.write();
        for i in 0..keys.len() {
            let key = keys[i].clone();
            let value = values[i].clone();
            cache.insert(key, value);
        }
        Ok(())
    }

    fn remove_batch(&self, _keys: &[Vec<u8>]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        let len = self.cache.read().len();
        let mut keys = Vec::with_capacity(len);
        let mut values = Vec::with_capacity(len);

        for (key, value) in self.cache.write().drain() {
            keys.push(key);
            values.push(value);
        }

        block_on(self.db.insert_batch(
            Context::new(),
            DataCategory::State,
            keys.to_vec(),
            values.to_vec(),
        ))
    }
}

impl<DB> Clone for TrieDB<DB>
where
    DB: Database,
{
    fn clone(&self) -> Self {
        TrieDB {
            db:    Arc::clone(&self.db),
            cache: Arc::clone(&self.cache),
        }
    }
}
