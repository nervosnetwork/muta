use core_runtime::{
    pool::Order,
    {Context, FutRuntimeResult},
};
use core_types::{transaction::SignedTransaction, Hash};

use crate::errors::OrderError;

#[derive(Debug)]
pub struct FIFO {}

impl FIFO {
    pub fn new() -> Self {
        FIFO {}
    }
}

impl Order for FIFO {
    type Error = OrderError;

    fn insert(
        &mut self,
        ctx: &Context,
        signed_tx: SignedTransaction,
    ) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn get_batch(&mut self, ctx: &Context, count: u64) -> FutRuntimeResult<&[Hash], Self::Error> {
        unimplemented!()
    }

    fn flush(&mut self, ctx: &Context, hashes: &[&Hash]) -> FutRuntimeResult<(), Self::Error> {
        unimplemented!()
    }

    fn get(
        &mut self,
        ctx: &Context,
        hash: &Hash,
    ) -> FutRuntimeResult<&[&SignedTransaction], Self::Error> {
        unimplemented!()
    }
}
