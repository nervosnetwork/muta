use std::collections::HashMap;
use std::sync::RwLock;

use core_runtime::{Context, Database, FutRuntimeResult};

use crate::errors::MemoryDBError;

#[derive(Default)]
pub struct MemoryDB {
    storage: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

impl Database for MemoryDB {
    type Error = MemoryDBError;

    fn get(&self, ctx: &Context, key: &[u8]) -> FutRuntimeResult<Option<Vec<u8>>, Self::Error> {
        unimplemented!()
    }

    fn get_batch(
        &self,
        ctx: &Context,
        keys: &[&[u8]],
    ) -> FutRuntimeResult<Vec<Option<Vec<u8>>>, Self::Error> {
        unimplemented!()
    }

    fn insert(
        &mut self,
        ctx: &Context,
        key: &[u8],
        value: &[u8],
    ) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn insert_batch(
        &mut self,
        ctx: &Context,
        keys: &[&[u8]],
        values: &[&[u8]],
    ) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn contain(&self, ctx: &Context, key: &[u8]) -> FutRuntimeResult<bool, Self::Error> {
        unimplemented!()
    }

    fn remove(&mut self, ctx: &Context, key: &[u8]) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn remove_batch(&mut self, ctx: &Context, keys: &[&[u8]]) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }
}
