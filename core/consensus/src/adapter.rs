use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;

use protocol::traits::executor::{ExecutorExecResp, ExecutorFactory, TrieDB};
use protocol::traits::{
    ConsensusAdapter, Context, CurrentConsensusStatus, Gossip, MemPool, MessageTarget,
    MixedTxHashes, NodeInfo, Priority, Storage,
};
use protocol::types::{Address, Epoch, Hash, Proof, Receipt, SignedTransaction, Validator};
use protocol::ProtocolResult;

pub struct OverlordConsensusAdapter<
    EF: ExecutorFactory<DB>,
    G: Gossip,
    M: MemPool,
    S: Storage,
    DB: TrieDB,
> {
    network: Arc<G>,
    mempool: Arc<M>,
    storage: Arc<S>,
    trie_db: Arc<DB>,

    pin_ef: PhantomData<EF>,
}

#[async_trait]
impl<EF, G, M, S, DB> ConsensusAdapter for OverlordConsensusAdapter<EF, G, M, S, DB>
where
    EF: ExecutorFactory<DB>,
    G: Gossip + Sync + Send,
    M: MemPool,
    S: Storage,
    DB: TrieDB,
{
    async fn get_txs_from_mempool(
        &self,
        ctx: Context,
        _epoch_id: u64,
        cycle_limit: u64,
    ) -> ProtocolResult<MixedTxHashes> {
        self.mempool.package(ctx, cycle_limit).await
    }

    async fn check_txs(&self, ctx: Context, check_txs: Vec<Hash>) -> ProtocolResult<()> {
        self.mempool.ensure_order_txs(ctx, check_txs).await
    }

    async fn sync_txs(&self, ctx: Context, txs: Vec<Hash>) -> ProtocolResult<()> {
        self.mempool.sync_propose_txs(ctx, txs).await
    }

    async fn get_full_txs(
        &self,
        ctx: Context,
        txs: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        self.mempool.get_full_txs(ctx, txs).await
    }

    async fn transmit(
        &self,
        ctx: Context,
        msg: Vec<u8>,
        end: &str,
        target: MessageTarget,
    ) -> ProtocolResult<()> {
        match target {
            MessageTarget::Broadcast => {
                self.network
                    .broadcast(ctx.clone(), end, msg, Priority::High)
                    .await
            }

            MessageTarget::Specified(addr) => {
                self.network
                    .users_cast(ctx, end, vec![addr], msg, Priority::High)
                    .await
            }
        }
    }

    async fn execute(
        &self,
        _ctx: Context,
        node_info: NodeInfo,
        status: CurrentConsensusStatus,
        coinbase: Address,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<ExecutorExecResp> {
        let mut executor = EF::from_root(
            node_info.chain_id,
            status.state_root,
            Arc::clone(&self.trie_db),
            status.epoch_id,
            status.cycles_price,
            coinbase,
        )?;
        executor.exec(signed_txs)
    }

    async fn flush_mempool(&self, ctx: Context, txs: Vec<Hash>) -> ProtocolResult<()> {
        self.mempool.flush(ctx, txs).await
    }

    async fn save_epoch(&self, _ctx: Context, epoch: Epoch) -> ProtocolResult<()> {
        self.storage.insert_epoch(epoch).await
    }

    async fn save_receipts(&self, _ctx: Context, receipts: Vec<Receipt>) -> ProtocolResult<()> {
        self.storage.insert_receipts(receipts).await
    }

    async fn save_proof(&self, _ctx: Context, proof: Proof) -> ProtocolResult<()> {
        self.storage.update_latest_proof(proof).await
    }

    async fn save_signed_txs(
        &self,
        _ctx: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        self.storage.insert_transactions(signed_txs).await
    }

    async fn get_last_validators(
        &self,
        _ctx: Context,
        epoch_id: u64,
    ) -> ProtocolResult<Vec<Validator>> {
        let epoch = self.storage.get_epoch_by_epoch_id(epoch_id).await?;
        Ok(epoch.header.validators)
    }
}

impl<EF, G, M, S, DB> OverlordConsensusAdapter<EF, G, M, S, DB>
where
    EF: ExecutorFactory<DB>,
    G: Gossip + Sync + Send,
    M: MemPool,
    S: Storage,
    DB: TrieDB,
{
    pub fn new(network: Arc<G>, mempool: Arc<M>, storage: Arc<S>, trie_db: Arc<DB>) -> Self {
        OverlordConsensusAdapter {
            network,
            mempool,
            storage,
            trie_db,

            pin_ef: PhantomData,
        }
    }
}
