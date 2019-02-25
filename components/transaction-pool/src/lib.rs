pub mod errors;
pub mod order;
pub mod verifier;

use core_runtime::{Context, Database, FutRuntimeResult, Order, Verifier};
use core_storage::storage::Storage;
use core_types::{Hash, SignedTransaction, UnverifiedTransaction};

use crate::errors::TransactionPoolError;

pub struct TransactionPool<'db, DB, O, V>
where
    DB: Database,
    O: Order,
    V: Verifier,
{
    storage: Storage<'db, DB>,

    order: O,
    verifier: V,
}

impl<'db, DB, O, V> TransactionPool<'db, DB, O, V>
where
    DB: Database,
    O: Order,
    V: Verifier,
{
    pub fn new(storage: Storage<'db, DB>, order: O, verifier: V) -> Self {
        TransactionPool {
            storage: storage,

            order: order,
            verifier: verifier,
        }
    }

    pub fn add(
        &self,
        ctx: &Context,
        untx: &UnverifiedTransaction,
    ) -> FutRuntimeResult<SignedTransaction, TransactionPoolError> {
        unimplemented!()
    }

    pub fn package(
        &mut self,
        ctx: &Context,
        count_limit: u64,
        quota_limit: u64,
    ) -> FutRuntimeResult<[SignedTransaction], TransactionPoolError> {
        unimplemented!()
    }

    pub fn clean(
        &mut self,
        ctx: &Context,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<(), TransactionPoolError> {
        unimplemented!()
    }

    pub fn check(
        &self,
        ctx: &Context,
        hashes: &[&Hash],
    ) -> FutRuntimeResult<bool, TransactionPoolError> {
        unimplemented!()
    }
}
