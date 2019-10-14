use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;

use protocol::traits::executor::{ExecutorFactory, ReadonlyResp, TrieDB};
use protocol::traits::{APIAdapter, Context, MemPool, Storage};
use protocol::types::{
    Address, AssetID, Balance, ContractAddress, Epoch, Hash, Receipt, SignedTransaction,
};
use protocol::ProtocolResult;

pub struct DefaultAPIAdapter<M, S, DB, EF>
where
    EF: ExecutorFactory<DB>,
    M: MemPool,
    S: Storage,
    DB: TrieDB,
{
    mempool: Arc<M>,
    storage: Arc<S>,
    trie_db: Arc<DB>,

    pin_ef: PhantomData<EF>,
}

impl<M, S, DB, EF> DefaultAPIAdapter<M, S, DB, EF>
where
    EF: ExecutorFactory<DB>,
    M: MemPool,
    S: Storage,
    DB: TrieDB,
{
    pub fn new(mempool: Arc<M>, storage: Arc<S>, trie_db: Arc<DB>) -> Self {
        Self {
            mempool,
            storage,
            trie_db,
            pin_ef: PhantomData,
        }
    }
}

#[async_trait]
impl<M, S, DB, EF> APIAdapter for DefaultAPIAdapter<M, S, DB, EF>
where
    EF: ExecutorFactory<DB>,
    M: MemPool,
    S: Storage,
    DB: TrieDB,
{
    async fn insert_signed_txs(
        &self,
        ctx: Context,
        signed_tx: SignedTransaction,
    ) -> ProtocolResult<()> {
        self.mempool.insert(ctx, signed_tx).await
    }

    async fn get_latest_epoch(&self, _ctx: Context) -> ProtocolResult<Epoch> {
        self.storage.get_latest_epoch().await
    }

    async fn get_epoch_by_id(&self, _ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        self.storage.get_epoch_by_epoch_id(epoch_id).await
    }

    async fn get_receipt_by_tx_hash(
        &self,
        _ctx: Context,
        tx_hash: Hash,
    ) -> ProtocolResult<Receipt> {
        self.storage.get_receipt(tx_hash).await
    }

    async fn get_balance(
        &self,
        _ctx: Context,
        address: &Address,
        id: &AssetID,
    ) -> ProtocolResult<Balance> {
        let epoch: Epoch = self.storage.get_latest_epoch().await?;

        let executor = EF::from_root(
            epoch.header.chain_id,
            epoch.header.state_root.clone(),
            Arc::clone(&self.trie_db),
            epoch.header.epoch_id,
            0,
            Address::User(epoch.header.proposer.clone()),
        )?;

        executor.get_balance(address, id)
    }

    async fn readonly(
        &self,
        _ctx: Context,
        epoch_id: Option<u64>,
        contract: ContractAddress,
        method: String,
        args: Vec<Bytes>,
    ) -> ProtocolResult<ReadonlyResp> {
        let epoch: Epoch = match epoch_id {
            None => self.storage.get_latest_epoch().await,
            Some(real_epoch_id) => self.storage.get_epoch_by_epoch_id(real_epoch_id).await,
        }?;
        let mut executor = EF::from_root(
            epoch.header.chain_id,
            epoch.header.state_root.clone(),
            Arc::clone(&self.trie_db),
            epoch.header.epoch_id,
            0,
            Address::User(epoch.header.proposer.clone()),
        )?;
        executor.readonly(contract, method, args)
    }
}
