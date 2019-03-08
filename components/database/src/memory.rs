use std::collections::HashMap;
use std::sync::RwLock;

use core_runtime::{Database, DatabaseError, FutRuntimeResult};

// TODO: remove this
#[allow(dead_code)]
#[derive(Default)]
pub struct MemoryDB {
    storage: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

impl Database for MemoryDB {
    fn get(&self, _key: &[u8]) -> FutRuntimeResult<Vec<u8>, DatabaseError> {
        unimplemented!()
    }

    fn get_batch(
        &self,
        _keys: &[Vec<u8>],
    ) -> FutRuntimeResult<Vec<Option<Vec<u8>>>, DatabaseError> {
        unimplemented!()
    }

    fn insert(&mut self, _key: &[u8], _value: &[u8]) -> FutRuntimeResult<(), DatabaseError> {
        unimplemented!()
    }

    fn insert_batch(
        &mut self,
        _keys: &[Vec<u8>],
        _values: &[Vec<u8>],
    ) -> FutRuntimeResult<(), DatabaseError> {
        unimplemented!()
    }

    fn contain(&self, _key: &[u8]) -> FutRuntimeResult<bool, DatabaseError> {
        unimplemented!()
    }

    fn remove(&mut self, _key: &[u8]) -> FutRuntimeResult<(), DatabaseError> {
        unimplemented!()
    }

    fn remove_batch(&mut self, _keys: &[Vec<u8>]) -> FutRuntimeResult<(), DatabaseError> {
        unimplemented!()
    }
}
