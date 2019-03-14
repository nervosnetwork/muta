pub mod errors;
pub mod order;
pub mod verifier;

use core_runtime::{Context, FutRuntimeResult, Order, Verifier};
use core_types::{Hash, SignedTransaction, UnverifiedTransaction};

use crate::errors::TransactionPoolError;

// TODO: remove this
#[allow(dead_code)]
pub struct TransactionPool<O, V>
where
    O: Order,
    V: Verifier,
{
    order: O,
    verifier: V,
}

impl<O, V> TransactionPool<O, V>
where
    O: Order,
    V: Verifier,
{
    pub fn new(order: O, verifier: V) -> Self {
        TransactionPool {
            order,
            verifier,
        }
    }

    pub fn add(
        &self,
        _ctx: &Context,
        _untx: &UnverifiedTransaction,
    ) -> FutRuntimeResult<SignedTransaction, TransactionPoolError> {
        unimplemented!()
    }

    pub fn package(
        &mut self,
        _ctx: &Context,
        _count_limit: u64,
        _quota_limit: u64,
    ) -> FutRuntimeResult<[SignedTransaction], TransactionPoolError> {
        unimplemented!()
    }

    pub fn clean(
        &mut self,
        _ctx: &Context,
        _hashes: &[&Hash],
    ) -> FutRuntimeResult<(), TransactionPoolError> {
        unimplemented!()
    }

    pub fn check(
        &self,
        _ctx: &Context,
        _hashes: &[&Hash],
    ) -> FutRuntimeResult<bool, TransactionPoolError> {
        unimplemented!()
    }
}
