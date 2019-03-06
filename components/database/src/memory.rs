use std::collections::HashMap;
use std::sync::RwLock;

use core_runtime::{Context, Database, FutRuntimeResult};

use crate::errors::MemoryDBError;

// TODO: remove this
#[allow(dead_code)]
#[derive(Default)]
pub struct MemoryDB {
    storage: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

impl Database for MemoryDB {
    type Error = MemoryDBError;

    fn get(&self, _ctx: &Context, _key: &[u8]) -> FutRuntimeResult<Option<Vec<u8>>, Self::Error> {
        unimplemented!()
    }

    fn get_batch(
        &self,
        _ctx: &Context,
        _keys: &[&[u8]],
    ) -> FutRuntimeResult<Vec<Option<Vec<u8>>>, Self::Error> {
        unimplemented!()
    }

    fn insert(
        &mut self,
        _ctx: &Context,
        _key: &[u8],
        _value: &[u8],
    ) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn insert_batch(
        &mut self,
        _ctx: &Context,
        _keys: &[&[u8]],
        _values: &[&[u8]],
    ) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn contain(&self, _ctx: &Context, _key: &[u8]) -> FutRuntimeResult<bool, Self::Error> {
        unimplemented!()
    }

    fn remove(&mut self, _ctx: &Context, _key: &[u8]) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn remove_batch(
        &mut self,
        _ctx: &Context,
        _keys: &[&[u8]],
    ) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }
}
