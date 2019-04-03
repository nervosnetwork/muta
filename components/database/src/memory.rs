use std::collections::HashMap;
use std::sync::Arc;

use futures::future::{err, ok, Future};
use futures_locks::RwLock;

use core_runtime::{DataCategory, Database, DatabaseError, FutRuntimeResult};

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
    fn get(&self, c: DataCategory, key: &[u8]) -> FutRuntimeResult<Vec<u8>, DatabaseError> {
        let key = gen_key(&c, key);

        let fut = self
            .storage
            .read()
            .map_err(|()| DatabaseError::Internal("rwlock error".to_string()))
            .and_then(move |storage| match storage.get(&key) {
                Some(v) => ok(v.to_vec()),
                None => err(DatabaseError::NotFound),
            });

        Box::new(fut)
    }

    fn get_batch(
        &self,
        c: DataCategory,
        keys: &[Vec<u8>],
    ) -> FutRuntimeResult<Vec<Option<Vec<u8>>>, DatabaseError> {
        let keys = gen_keys(&c, keys);

        let fut = self
            .storage
            .read()
            .map_err(|()| DatabaseError::Internal("rwlock error".to_string()))
            .map(move |storage| {
                keys.into_iter()
                    .map(|key| storage.get(&key.to_vec()))
                    .map(|v| match v {
                        Some(v) => Some(v.to_vec()),
                        None => None,
                    })
                    .collect()
            });

        Box::new(fut)
    }

    fn insert(
        &self,
        c: DataCategory,
        key: &[u8],
        value: &[u8],
    ) -> FutRuntimeResult<(), DatabaseError> {
        let key = gen_key(&c, key);
        let value = value.to_vec();

        let fut = self
            .storage
            .write()
            .map_err(|()| DatabaseError::Internal("rwlock error".to_string()))
            .map(move |mut storage| storage.insert(key, value))
            .map(|_| ());

        Box::new(fut)
    }

    fn insert_batch(
        &self,
        c: DataCategory,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
    ) -> FutRuntimeResult<(), DatabaseError> {
        if keys.len() != values.len() {
            return Box::new(err(DatabaseError::InvalidData));
        }

        let keys = gen_keys(&c, keys);
        let values = values.to_vec();

        let fut = self
            .storage
            .write()
            .map_err(|()| DatabaseError::Internal("rwlock error".to_string()))
            .map(move |mut storage| {
                for i in 0..keys.len() {
                    let key = keys[i].to_vec();
                    let value = values[i].to_vec();

                    storage.insert(key, value);
                }
            });

        Box::new(fut)
    }

    fn contains(&self, c: DataCategory, key: &[u8]) -> FutRuntimeResult<bool, DatabaseError> {
        let key = gen_key(&c, key);

        let fut = self
            .storage
            .read()
            .map_err(|()| DatabaseError::Internal("rwlock error".to_string()))
            .map(move |storage| storage.contains_key(&key));

        Box::new(fut)
    }

    fn remove(&self, c: DataCategory, key: &[u8]) -> FutRuntimeResult<(), DatabaseError> {
        let key = gen_key(&c, key);

        let fut = self
            .storage
            .write()
            .map_err(|()| DatabaseError::Internal("rwlock error".to_string()))
            .map(move |mut storage| {
                storage.remove(&key);
            });

        Box::new(fut)
    }

    fn remove_batch(
        &self,
        c: DataCategory,
        keys: &[Vec<u8>],
    ) -> FutRuntimeResult<(), DatabaseError> {
        let keys = gen_keys(&c, keys);

        let fut = self
            .storage
            .write()
            .map_err(|()| DatabaseError::Internal("rwlock error".to_string()))
            .map(move |mut storage| {
                for key in keys {
                    storage.remove(&key);
                }
            });

        Box::new(fut)
    }
}

fn gen_key(c: &DataCategory, key: &[u8]) -> Vec<u8> {
    match c {
        DataCategory::Block => [b"block-", key].concat(),
        DataCategory::Transaction => [b"transaction-", key].concat(),
        DataCategory::Receipt => [b"receipt-", key].concat(),
        DataCategory::State => [b"state-", key].concat(),
        DataCategory::TransactionPool => [b"transaction-pool-", key].concat(),
        DataCategory::TransactionPosition => [b"transaction-position-", key].concat(),
    }
}

fn gen_keys(c: &DataCategory, keys: &[Vec<u8>]) -> Vec<Vec<u8>> {
    keys.iter().map(|key| gen_key(c, key)).collect()
}

#[cfg(test)]
mod tests {
    use super::MemoryDB;

    use core_runtime::{DataCategory, Database, DatabaseError};
    use futures::future::Future;

    #[test]
    fn test_get_should_return_ok() {
        let db = MemoryDB::new();

        check_not_found(db.get(DataCategory::Block, b"test").wait());
        db.insert(DataCategory::Block, b"test", b"test")
            .wait()
            .unwrap();
        let v = db.get(DataCategory::Block, b"test").wait().unwrap();
        assert_eq!(v, b"test".to_vec())
    }

    #[test]
    fn test_insert_should_return_ok() {
        let db = MemoryDB::new();

        db.insert(DataCategory::Block, b"test", b"test")
            .wait()
            .unwrap();
        assert_eq!(
            b"test".to_vec(),
            db.get(DataCategory::Block, b"test").wait().unwrap()
        );
    }

    #[test]
    fn test_insert_batch_should_return_ok() {
        let db = MemoryDB::new();

        db.insert_batch(
            DataCategory::Block,
            &[b"test1".to_vec(), b"test2".to_vec()],
            &[b"test1".to_vec(), b"test2".to_vec()],
        )
        .wait()
        .unwrap();
        assert_eq!(
            b"test1".to_vec(),
            db.get(DataCategory::Block, b"test1").wait().unwrap()
        );
        assert_eq!(
            b"test2".to_vec(),
            db.get(DataCategory::Block, b"test2").wait().unwrap()
        );
    }

    #[test]
    fn test_contain_should_return_true() {
        let db = MemoryDB::new();

        db.insert(DataCategory::Block, b"test", b"test")
            .wait()
            .unwrap();
        assert_eq!(
            db.contains(DataCategory::Block, b"test").wait().unwrap(),
            true
        )
    }

    #[test]
    fn test_contain_should_return_false() {
        let db = MemoryDB::new();
        assert_eq!(
            db.contains(DataCategory::Block, b"test").wait().unwrap(),
            false
        )
    }

    #[test]
    fn test_remove_should_return_ok() {
        let db = MemoryDB::new();

        db.insert(DataCategory::Block, b"test", b"test")
            .wait()
            .unwrap();
        db.remove(DataCategory::Block, b"test").wait().unwrap();
        check_not_found(db.get(DataCategory::Block, b"test").wait());
    }

    #[test]
    fn test_remove_batch_should_return_ok() {
        let db = MemoryDB::new();

        db.insert_batch(
            DataCategory::Block,
            &[b"test1".to_vec(), b"test2".to_vec()],
            &[b"test1".to_vec(), b"test2".to_vec()],
        )
        .wait()
        .unwrap();
        db.remove_batch(DataCategory::Block, &[b"test1".to_vec(), b"test2".to_vec()])
            .wait()
            .unwrap();
        check_not_found(db.get(DataCategory::Block, b"test1").wait());
        check_not_found(db.get(DataCategory::Block, b"test2").wait());
    }

    fn check_not_found<T>(res: Result<T, DatabaseError>) {
        match res {
            Ok(_) => panic!("The result must be an error not found"),
            Err(e) => assert_eq!(e, DatabaseError::NotFound),
        }
    }
}
