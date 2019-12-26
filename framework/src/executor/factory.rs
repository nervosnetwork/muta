use std::sync::Arc;

use protocol::traits::{Executor, ExecutorFactory, Storage};
use protocol::types::MerkleRoot;
use protocol::ProtocolResult;

use crate::executor::ServiceExecutor;

pub struct ServiceExecutorFactory;

impl<DB: 'static + cita_trie::DB, S: 'static + Storage> ExecutorFactory<DB, S>
    for ServiceExecutorFactory
{
    fn from_root(
        root: MerkleRoot,
        db: Arc<DB>,
        storage: Arc<S>,
    ) -> ProtocolResult<Box<dyn Executor>> {
        let executor = ServiceExecutor::with_root(root, db, storage)?;
        Ok(Box::new(executor))
    }
}
