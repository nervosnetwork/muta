use core_runtime::{Context, Database, FutRuntimeResult};
use core_types::{Block, Hash, Receipt, SignedTransaction};

use crate::errors::StorageError;

pub struct Storage<'db, DB>
where
    DB: Database,
{
    db: &'db mut DB,
}

impl<'db, DB> Storage<'db, DB>
where
    DB: Database,
{
    pub fn new(db: &'db mut DB) -> Self {
        Storage { db: db }
    }

    pub fn add_block(&mut self, ctx: &Context, block: Block) -> FutRuntimeResult<(), StorageError> {
        unimplemented!()
    }

    pub fn get_block_by_height(
        &self,
        ctx: &Context,
        height: u64,
    ) -> FutRuntimeResult<Block, StorageError> {
        unimplemented!()
    }

    pub fn get_block_by_hash(
        &self,
        ctx: &Context,
        h: Hash,
    ) -> FutRuntimeResult<Block, StorageError> {
        unimplemented!()
    }

    pub fn get_receipt(
        &self,
        ctx: &Context,
        tx_hash: Hash,
    ) -> FutRuntimeResult<Receipt, StorageError> {
        unimplemented!()
    }

    pub fn get_transaction(
        &self,
        ctx: &Context,
        hash: Hash,
    ) -> FutRuntimeResult<SignedTransaction, StorageError> {
        unimplemented!()
    }
}
