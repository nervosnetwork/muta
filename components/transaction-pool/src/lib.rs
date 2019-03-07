pub mod errors;
pub mod order;
pub mod verifier;

use core_runtime::{Context, Database, FutRuntimeResult, Order, Verifier};
use core_types::{Hash, SignedTransaction, UnverifiedTransaction};

use crate::errors::TransactionPoolError;

// TODO: remove this
#[allow(dead_code)]
pub struct TransactionPool<DB, O, V>
where
    DB: Database,
    O: Order,
    V: Verifier,
{
    storage: DB,

    order: O,
    verifier: V,
}

impl<DB, O, V> TransactionPool<DB, O, V>
where
    DB: Database,
    O: Order,
    V: Verifier,
{
    pub fn new(storage: DB, order: O, verifier: V) -> Self {
        TransactionPool {
            storage,

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
