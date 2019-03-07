use core_runtime::{
    pool::Order,
    {Context, FutRuntimeResult},
};
use core_types::{transaction::SignedTransaction, Hash};

use crate::errors::OrderError;

#[derive(Debug)]
pub struct FIFO {}

// TODO: remove this
#[allow(clippy::new_without_default)]
impl FIFO {
    pub fn new() -> Self {
        FIFO {}
    }
}

impl Order for FIFO {
    type Error = OrderError;

    fn insert(
        &mut self,
        _ctx: &Context,
        _signed_tx: SignedTransaction,
    ) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn get_batch(&mut self, _ctx: &Context, _count: u64) -> FutRuntimeResult<&[Hash], Self::Error> {
        unimplemented!()
    }

    fn flush(&mut self, _ctx: &Context, _hashes: &[&Hash]) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn get(
        &mut self,
        _ctx: &Context,
        _hash: &Hash,
    ) -> FutRuntimeResult<&[&SignedTransaction], Self::Error> {
        unimplemented!()
    }
}
