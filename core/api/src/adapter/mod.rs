use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;

use protocol::traits::ExecutorFactory;
use protocol::traits::{
    APIAdapter, Context, ExecResp, ExecutorParams, MemPool, ServiceMapping, Storage,
};
use protocol::types::{Address, Epoch, Hash, Receipt, SignedTransaction, TransactionRequest};
use protocol::ProtocolResult;

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

    async fn get_epoch_by_id(&self, _ctx: Context, epoch_id: Option<u64>) -> ProtocolResult<Epoch> {
        let epoch = match epoch_id {
            Some(id) => self.storage.get_epoch_by_epoch_id(id).await?,
            None => self.storage.get_latest_epoch().await?,
        };

        Ok(epoch)
    }

    async fn get_receipt_by_tx_hash(
        &self,
        _ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<Receipt> {
        self.storage.get_receipt(tx_hash).await
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
        epoch_id: u64,
        cycels_limit: u64,
        cycles_price: u64,
        caller: Address,
        service_name: String,
        method: String,
        payload: String,
    ) -> ProtocolResult<ExecResp> {
        let epoch = self.get_epoch_by_id(ctx.clone(), Some(epoch_id)).await?;

        let executor = EF::from_root(
            epoch.header.state_root.clone(),
            Arc::clone(&self.trie_db),
            Arc::clone(&self.storage),
            Arc::clone(&self.service_mapping),
        )?;

        let params = ExecutorParams {
            state_root: epoch.header.state_root,
            epoch_id,
            timestamp: epoch.header.timestamp,
            cycels_limit,
        };
        executor.read(&params, &caller, cycles_price, &TransactionRequest {
            service_name,
            method,
            payload,
        })
    }
}
