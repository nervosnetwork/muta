use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;

use protocol::traits::executor::{ExecutorFactory, TrieDB};
use protocol::traits::{APIAdapter, Context, MemPool, Storage};
use protocol::types::{Address, AssetID, Balance, Epoch, Hash, Receipt, SignedTransaction};
use protocol::ProtocolResult;

pub struct DefaultAPIAdapter<EF, M, S, DB> {
    mempool: Arc<M>,
    storage: Arc<S>,
    trie_db: Arc<DB>,

    pin_ef: PhantomData<EF>,
}

impl<EF: ExecutorFactory<DB>, M: MemPool, S: Storage, DB: TrieDB> DefaultAPIAdapter<EF, M, S, DB> {
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
impl<EF: ExecutorFactory<DB>, M: MemPool, S: Storage, DB: TrieDB> APIAdapter
    for DefaultAPIAdapter<EF, M, S, DB>
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

    async fn get_balance(
        &self,
        ctx: Context,
        address: &Address,
        id: &AssetID,
        epoch_id: Option<u64>,
    ) -> ProtocolResult<Balance> {
        let epoch: Epoch = self.get_epoch_by_id(ctx.clone(), epoch_id).await?;

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
}
