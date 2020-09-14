use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use derive_more::Display;

use protocol::traits::{
    APIAdapter, Context, ExecutorFactory, ExecutorParams, MemPool, ServiceMapping, ServiceResponse,
    Storage,
};
use protocol::types::{
    Address, Block, BlockHeader, Hash, Receipt, SignedTransaction, TransactionRequest,
};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[derive(Debug, Display)]
pub enum APIError {
    #[display(
        fmt = "Unexecuted block,try to {:?}, but now only reached {:?}",
        real,
        expect
    )]
    UnExecedError { expect: u64, real: u64 },

    #[display(fmt = "not found")]
    NotFound,
}

impl std::error::Error for APIError {}

impl From<APIError> for ProtocolError {
    fn from(api_err: APIError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::API, Box::new(api_err))
    }
}

pub struct DefaultAPIAdapter<EF, M, S, DB, Mapping> {
    mempool:         Arc<M>,
    storage:         Arc<S>,
    trie_db:         Arc<DB>,
    service_mapping: Arc<Mapping>,

    pin_ef: PhantomData<EF>,
}

impl<
        EF: ExecutorFactory<DB, S, Mapping>,
        M: MemPool,
        S: Storage,
        DB: cita_trie::DB,
        Mapping: ServiceMapping,
    > DefaultAPIAdapter<EF, M, S, DB, Mapping>
{
    pub fn new(
        mempool: Arc<M>,
        storage: Arc<S>,
        trie_db: Arc<DB>,
        service_mapping: Arc<Mapping>,
    ) -> Self {
        Self {
            mempool,
            storage,
            trie_db,
            service_mapping,
            pin_ef: PhantomData,
        }
    }
}

#[async_trait]
impl<
        EF: ExecutorFactory<DB, S, Mapping>,
        M: MemPool,
        S: Storage,
        DB: cita_trie::DB,
        Mapping: ServiceMapping,
    > APIAdapter for DefaultAPIAdapter<EF, M, S, DB, Mapping>
{
    async fn insert_signed_txs(
        &self,
        ctx: Context,
        signed_tx: SignedTransaction,
    ) -> ProtocolResult<()> {
        self.mempool.insert(ctx, signed_tx).await
    }

    async fn get_block_by_height(
        &self,
        ctx: Context,
        height: Option<u64>,
    ) -> ProtocolResult<Option<Block>> {
        match height {
            Some(id) => self.storage.get_block(ctx.clone(), id).await,
            None => Ok(Some(self.storage.get_latest_block(ctx).await?)),
        }
    }

    async fn get_block_header_by_height(
        &self,
        ctx: Context,
        height: Option<u64>,
    ) -> ProtocolResult<Option<BlockHeader>> {
        match height {
            Some(id) => self.storage.get_block_header(ctx.clone(), id).await,
            None => Ok(Some(self.storage.get_latest_block_header(ctx).await?)),
        }
    }

    async fn get_receipt_by_tx_hash(
        &self,
        ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<Receipt>> {
        let opt_receipt = self
            .storage
            .get_receipt_by_hash(ctx.clone(), tx_hash)
            .await?;

        let exec_height = self.storage.get_latest_block_header(ctx).await?.exec_height;

        match opt_receipt {
            Some(receipt) => {
                let height = receipt.height;
                if exec_height >= height {
                    Ok(Some(receipt))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn get_transaction_by_hash(
        &self,
        ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>> {
        self.storage.get_transaction_by_hash(ctx, tx_hash).await
    }

    async fn query_service(
        &self,
        ctx: Context,
        height: u64,
        cycles_limit: u64,
        cycles_price: u64,
        caller: Address,
        service_name: String,
        method: String,
        payload: String,
    ) -> ProtocolResult<ServiceResponse<String>> {
        let header = self
            .get_block_header_by_height(ctx.clone(), Some(height))
            .await?
            .ok_or_else(|| APIError::NotFound)?;

        let executor = EF::from_root(
            header.state_root.clone(),
            Arc::clone(&self.trie_db),
            Arc::clone(&self.storage),
            Arc::clone(&self.service_mapping),
        )?;

        let params = ExecutorParams {
            state_root: header.state_root,
            height,
            timestamp: header.timestamp,
            cycles_limit,
            proposer: header.proposer,
        };
        executor.read(&params, &caller, cycles_price, &TransactionRequest {
            service_name,
            method,
            payload,
        })
    }
}
