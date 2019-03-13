use std::collections::HashMap;

use futures::future::{err, ok, Future};
use futures_locks::RwLock;

use core_runtime::{Database, DatabaseError, FutRuntimeResult};

pub struct MemoryDB {
    storage: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

impl MemoryDB {
    pub fn new() -> Self {
        MemoryDB {
            storage: RwLock::new(HashMap::new()),
        }
    }
}

impl Database for MemoryDB {
    fn get(&self, key: &[u8]) -> FutRuntimeResult<Vec<u8>, DatabaseError> {
        let key = key.to_vec();

        let fut = self
            .storage
            .read()
            .map_err(|()| DatabaseError::Internal)
            .and_then(move |storage| match storage.get(&key) {
                Some(v) => ok(v.to_vec()),
                None => err(DatabaseError::NotFound),
            });

        Box::new(fut)
    }

    fn get_batch(&self, keys: &[Vec<u8>]) -> FutRuntimeResult<Vec<Option<Vec<u8>>>, DatabaseError> {
        let keys = keys.to_vec();

        let fut = self
            .storage
            .read()
            .map_err(|()| DatabaseError::Internal)
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

    fn insert(&mut self, key: &[u8], value: &[u8]) -> FutRuntimeResult<(), DatabaseError> {
        let key = key.to_vec();
        let value = value.to_vec();

        let fut = self
            .storage
            .write()
            .map_err(|()| DatabaseError::Internal)
            .map(move |mut storage| storage.insert(key, value))
            .map(|_| ());

        Box::new(fut)
    }

    fn insert_batch(
        &mut self,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
    ) -> FutRuntimeResult<(), DatabaseError> {
        if keys.len() != values.len() {
            return Box::new(err(DatabaseError::InvalidData));
        }

        let keys = keys.to_vec();
        let values = values.to_vec();

        let fut = self
            .storage
            .write()
            .map_err(|()| DatabaseError::Internal)
            .map(move |mut storage| {
                for i in 0..keys.len() {
                    let key = keys[i].to_vec();
                    let value = values[i].to_vec();

                    storage.insert(key, value);
                }
                ()
            });

        Box::new(fut)
    }

    fn contain(&self, key: &[u8]) -> FutRuntimeResult<bool, DatabaseError> {
        let key = key.to_vec();

        let fut = self
            .storage
            .read()
            .map_err(|()| DatabaseError::Internal)
            .map(move |storage| storage.contains_key(&key));

        Box::new(fut)
    }

    fn remove(&mut self, key: &[u8]) -> FutRuntimeResult<(), DatabaseError> {
        let key = key.to_vec();

        let fut = self
            .storage
            .write()
            .map_err(|()| DatabaseError::Internal)
            .map(move |mut storage| {
                storage.remove(&key);
                ()
            });

        Box::new(fut)
    }

    fn remove_batch(&mut self, keys: &[Vec<u8>]) -> FutRuntimeResult<(), DatabaseError> {
        let keys = keys.to_vec();

        let fut = self
            .storage
            .write()
            .map_err(|()| DatabaseError::Internal)
            .map(move |mut storage| {
                for key in keys {
                    storage.remove(&key);
                }
                ()
            });

        Box::new(fut)
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryDB;

    use core_runtime::{Database, DatabaseError};
    use futures::future::Future;

    #[test]
    fn test_get_should_return_ok() {
        let mut memdb = MemoryDB::new();
        check_not_found(memdb.get(b"test").wait());
        memdb.insert(b"test", b"test").wait().unwrap();
        let v = memdb.get(b"test").wait().unwrap();
        assert_eq!(v, b"test".to_vec())
    }

    #[test]
    fn test_insert_should_return_ok() {
        let mut memdb = MemoryDB::new();
        memdb.insert(b"test", b"test").wait().unwrap();
        assert_eq!(b"test".to_vec(), memdb.get(b"test").wait().unwrap());
    }

    #[test]
    fn test_insert_batch_should_return_ok() {
        let mut memdb = MemoryDB::new();
        memdb
            .insert_batch(
                &[b"test1".to_vec(), b"test2".to_vec()],
                &[b"test1".to_vec(), b"test2".to_vec()],
            )
            .wait()
            .unwrap();
        assert_eq!(b"test1".to_vec(), memdb.get(b"test1").wait().unwrap());
        assert_eq!(b"test2".to_vec(), memdb.get(b"test2").wait().unwrap());
    }

    #[test]
    fn test_contain_should_return_true() {
        let mut memdb = MemoryDB::new();
        memdb.insert(b"test", b"test").wait().unwrap();
        assert_eq!(memdb.contain(b"test").wait().unwrap(), true)
    }

    #[test]
    fn test_contain_should_return_false() {
        let memdb = MemoryDB::new();
        assert_eq!(memdb.contain(b"test").wait().unwrap(), false)
    }

    #[test]
    fn test_remove_should_return_ok() {
        let mut memdb = MemoryDB::new();
        memdb.insert(b"test", b"test").wait().unwrap();
        memdb.remove(b"test").wait().unwrap();
        check_not_found(memdb.get(b"test").wait());
    }

    #[test]
    fn test_remove_batch_should_return_ok() {
        let mut memdb = MemoryDB::new();
        memdb
            .insert_batch(
                &[b"test1".to_vec(), b"test2".to_vec()],
                &[b"test1".to_vec(), b"test2".to_vec()],
            )
            .wait()
            .unwrap();
        memdb
            .remove_batch(&[b"test1".to_vec(), b"test2".to_vec()])
            .wait()
            .unwrap();
        check_not_found(memdb.get(b"test1").wait());
        check_not_found(memdb.get(b"test2").wait());
    }

    fn check_not_found<T>(res: Result<T, DatabaseError>) {
        match res {
            Ok(_) => panic!("The result must be an error not found"),
            Err(e) => assert_eq!(e, DatabaseError::NotFound),
        }
    }
}
