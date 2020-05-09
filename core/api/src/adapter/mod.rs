mod schema;

use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Once};

use async_trait::async_trait;
use derive_more::Display;

use protocol::traits::ExecutorFactory;
use protocol::traits::{
    APIAdapter, Context, ExecutorParams, MemPool, ServiceMapping, ServiceResponse, Storage,
};
use protocol::types::{
    Address, Block, ChainSchema, Hash, Receipt, SignedTransaction, TransactionRequest,
};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use schema::*;

static SCHEMA_ONCE: Once = Once::new();

#[derive(Debug, Display)]
pub enum APIError {
    #[display(
        fmt = "Unexecuted block,try to {:?}, but now only reached {:?}",
        real,
        expect
    )]
    UnExecedError {
        expect: u64,
        real:   u64,
    },

    SchemaError(String),
}

impl std::error::Error for APIError {}

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
        _ctx: Context,
        height: Option<u64>,
    ) -> ProtocolResult<Block> {
        let block = match height {
            Some(id) => self.storage.get_block_by_height(id).await?,
            None => self.storage.get_latest_block().await?,
        };

        Ok(block)
    }

    async fn get_receipt_by_tx_hash(
        &self,
        _ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<Receipt> {
        let receipt = self.storage.get_receipt(tx_hash).await?;
        let exec_height = self.storage.get_latest_block().await?.header.exec_height;
        let height = receipt.height;
        if exec_height >= height {
            return Ok(receipt);
        }
        Err(ProtocolError::new(
            ProtocolErrorKind::API,
            Box::new(APIError::UnExecedError {
                real:   exec_height,
                expect: height,
            }),
        ))
    }

    async fn get_transaction_by_hash(
        &self,
        _: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<SignedTransaction> {
        self.storage.get_transaction_by_hash(tx_hash).await
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
        let block = self.get_block_by_height(ctx.clone(), Some(height)).await?;

        let executor = EF::from_root(
            block.header.state_root.clone(),
            Arc::clone(&self.trie_db),
            Arc::clone(&self.storage),
            Arc::clone(&self.service_mapping),
        )?;

        let params = ExecutorParams {
            state_root: block.header.state_root,
            height,
            timestamp: block.header.timestamp,
            cycles_limit,
        };
        executor.read(&params, &caller, cycles_price, &TransactionRequest {
            service_name,
            method,
            payload,
        })
    }

    async fn get_schema(&self, ctx: Context) -> ProtocolResult<&ChainSchema> {
        static mut CHAIN_SCHEMA: Option<ChainSchema> = None;
        let block = self.get_block_by_height(ctx.clone(), None).await?;
        unsafe {
            panic::catch_unwind(AssertUnwindSafe(|| {
                SCHEMA_ONCE.call_once(|| {
                    let executor = EF::from_root(
                        block.header.state_root.clone(),
                        Arc::clone(&self.trie_db),
                        Arc::clone(&self.storage),
                        Arc::clone(&self.service_mapping),
                    )
                    .expect("Api get schema: executor from root error");
                    CHAIN_SCHEMA = Some(lazy_schema(
                        executor
                            .get_service_metas()
                            .expect("Api get schema: executor get service metas error"),
                    ));
                });
            }))
            .map_err(|e| {
                ProtocolError::new(
                    ProtocolErrorKind::API,
                    Box::new(APIError::SchemaError(format!(
                        "Api get schema error: {:?}",
                        e
                    ))),
                )
            })?;

            CHAIN_SCHEMA.as_ref().ok_or(ProtocolError::new(
                ProtocolErrorKind::API,
                Box::new(APIError::SchemaError(format!(
                    "Api get schema error: get none"
                ))),
            ))
        }
    }
}
