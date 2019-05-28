use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use core_context::Context;
use core_runtime::{DataCategory, Database, DatabaseError, FutDBResult};

pub struct MemoryDB {
    storage: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryDB {
    pub fn new() -> Self {
        MemoryDB {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryDB {
    fn default() -> Self {
        MemoryDB {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Database for MemoryDB {
    fn get(&self, _: Context, c: DataCategory, key: &[u8]) -> FutDBResult<Option<Vec<u8>>> {
        let storage = Arc::clone(&self.storage);
        let key = gen_key(&c, key.to_vec());

        let fut = async move {
            let storage = storage.read().map_err(|_| map_rwlock_err())?;
            let v = storage.get(&key).map(|v| v.to_vec());
            Ok(v)
        };

        Box::pin(fut)
    }

    fn get_batch(
        &self,
        _: Context,
        c: DataCategory,
        keys: &[Vec<u8>],
    ) -> FutDBResult<Vec<Option<Vec<u8>>>> {
        let storage = Arc::clone(&self.storage);
        let keys = gen_keys(&c, keys.to_vec());

        let fut = async move {
            let storage = storage.read().map_err(|_| map_rwlock_err())?;
            let values = keys
                .into_iter()
                .map(|key| storage.get(&key.to_vec()).map(|v| v.to_vec()))
                .collect();

            Ok(values)
        };

        Box::pin(fut)
    }

    fn insert(&self, _: Context, c: DataCategory, key: Vec<u8>, value: Vec<u8>) -> FutDBResult<()> {
        let storage = Arc::clone(&self.storage);
        let key = gen_key(&c, key);
        let value = value.to_vec();

        let fut = async move {
            let mut storage = storage.write().map_err(|_| map_rwlock_err())?;
            storage.insert(key, value);
            Ok(())
        };

        Box::pin(fut)
    }

    fn insert_batch(
        &self,
        _: Context,
        c: DataCategory,
        keys: Vec<Vec<u8>>,
        values: Vec<Vec<u8>>,
    ) -> FutDBResult<()> {
        let storage = Arc::clone(&self.storage);
        let keys = gen_keys(&c, keys);
        let values = values.to_vec();

        let fut = async move {
            if keys.len() != values.len() {
                return Err(DatabaseError::InvalidData);
            }

            let mut storage = storage.write().map_err(|_| map_rwlock_err())?;
            for i in 0..keys.len() {
                let key = keys[i].to_vec();
                let value = values[i].to_vec();

                storage.insert(key, value);
            }

            Ok(())
        };

        Box::pin(fut)
    }

    fn contains(&self, _: Context, c: DataCategory, key: &[u8]) -> FutDBResult<bool> {
        let storage = Arc::clone(&self.storage);
        let key = gen_key(&c, key.to_vec());

        let fut = async move {
            let storage = storage.read().map_err(|_| map_rwlock_err())?;
            Ok(storage.contains_key(&key))
        };

        Box::pin(fut)
    }

    fn remove(&self, _: Context, c: DataCategory, key: &[u8]) -> FutDBResult<()> {
        let storage = Arc::clone(&self.storage);
        let key = gen_key(&c, key.to_vec());

        let fut = async move {
            let mut storage = storage.write().map_err(|_| map_rwlock_err())?;
            storage.remove(&key);
            Ok(())
        };

        Box::pin(fut)
    }

    fn remove_batch(&self, _: Context, c: DataCategory, keys: &[Vec<u8>]) -> FutDBResult<()> {
        let storage = Arc::clone(&self.storage);
        let keys = gen_keys(&c, keys.to_vec());

        let fut = async move {
            let mut storage = storage.write().map_err(|_| map_rwlock_err())?;
            for key in keys {
                storage.remove(&key);
            }
            Ok(())
        };

        Box::pin(fut)
    }
}

fn gen_key(c: &DataCategory, key: Vec<u8>) -> Vec<u8> {
    match c {
        DataCategory::Block => [b"block-".to_vec(), key].concat(),
        DataCategory::Transaction => [b"transaction-".to_vec(), key].concat(),
        DataCategory::Receipt => [b"receipt-".to_vec(), key].concat(),
        DataCategory::State => [b"state-".to_vec(), key].concat(),
        DataCategory::TransactionPool => [b"transaction-pool-".to_vec(), key].concat(),
        DataCategory::TransactionPosition => [b"transaction-position-".to_vec(), key].concat(),
    }
}

fn gen_keys(c: &DataCategory, keys: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    keys.into_iter().map(|key| gen_key(c, key)).collect()
}

fn map_rwlock_err() -> DatabaseError {
    DatabaseError::Internal("rwlock error".to_string())
}

#[cfg(test)]
mod tests {
    use crate::test::{
        test_contains, test_get, test_insert, test_insert_batch, test_remove, test_remove_batch,
    };

    use super::MemoryDB;

    #[test]
    fn test_get_should_pass() {
        let db = MemoryDB::new();

        test_get(&db);
    }

    #[test]
    fn test_insert_should_pass() {
        let db = MemoryDB::new();

        test_insert(&db);
    }

    #[test]
    fn test_insert_batch_should_pass() {
        let db = MemoryDB::new();

        test_insert_batch(&db);
    }

    #[test]
    fn test_contains_should_pass() {
        let db = MemoryDB::new();

        test_contains(&db);
    }

    #[test]
    fn test_remove_should_pass() {
        let db = MemoryDB::new();

        test_remove(&db);
    }

    #[test]
    fn test_remove_batch_should_pass() {
        let db = MemoryDB::new();

        test_remove_batch(&db);
    }
}
