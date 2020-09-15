use std::sync::Arc;

use derive_more::{Display, From};
use futures::executor::block_on;

use protocol::traits::{ChainQuerier, Context, Storage};
use protocol::types::{Block, Hash, Receipt, SignedTransaction};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub struct DefaultChainQuerier<S: Storage> {
    storage: Arc<S>,
}

impl<S: Storage> DefaultChainQuerier<S> {
    pub fn new(storage: Arc<S>) -> Self {
        Self { storage }
    }
}

impl<S: Storage> ChainQuerier for DefaultChainQuerier<S> {
    fn get_transaction_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<SignedTransaction>> {
        let ret = block_on(
            self.storage
                .get_transaction_by_hash(Context::new(), &tx_hash),
        )
        .map_err(|_| ChainQueryError::AsyncStorage)?;

        Ok(ret)
    }

    fn get_block_by_height(&self, height: Option<u64>) -> ProtocolResult<Option<Block>> {
        if let Some(u) = height {
            let ret = block_on(self.storage.get_block(Context::new(), u))
                .map_err(|_| ChainQueryError::AsyncStorage)?;

            Ok(ret)
        } else {
            let ret = block_on(self.storage.get_latest_block(Context::new()))
                .map_err(|_| ChainQueryError::AsyncStorage)?;

            Ok(Some(ret))
        }
    }

    fn get_receipt_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<Receipt>> {
        let ret = block_on(
            self.storage
                .get_receipt_by_hash(Context::new(), tx_hash.clone()),
        )
        .map_err(|_| ChainQueryError::AsyncStorage)?;

        Ok(ret)
    }
}

#[derive(Debug, Display, From)]
pub enum ChainQueryError {
    #[display(fmt = "get error when call async method of storage")]
    AsyncStorage,
}

impl std::error::Error for ChainQueryError {}

impl From<ChainQueryError> for ProtocolError {
    fn from(err: ChainQueryError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}
