use crate::{Context, FutRuntimeResult};

pub trait Database: Send + Sync {
    type Error;

    fn get(&self, ctx: &Context, key: &[u8]) -> FutRuntimeResult<Option<Vec<u8>>, Self::Error>;

    fn get_batch(
        &self,
        ctx: &Context,
        keys: &[&[u8]],
    ) -> FutRuntimeResult<Vec<Option<Vec<u8>>>, Self::Error>;

    fn insert(
        &mut self,
        ctx: &Context,
        key: &[u8],
        value: &[u8],
    ) -> FutRuntimeResult<(), Self::Error>;

    fn insert_batch(
        &mut self,
        ctx: &Context,
        keys: &[&[u8]],
        values: &[&[u8]],
    ) -> FutRuntimeResult<(), Self::Error>;

    fn contain(&self, ctx: &Context, key: &[u8]) -> FutRuntimeResult<bool, Self::Error>;

    fn remove(&mut self, ctx: &Context, key: &[u8]) -> FutRuntimeResult<(), Self::Error>;

    fn remove_batch(&mut self, ctx: &Context, keys: &[&[u8]]) -> FutRuntimeResult<(), Self::Error>;
}
