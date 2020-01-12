use std::boxed::Box;
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::stream::StreamExt;

use protocol::traits::{
    CommonConsensusAdapter, ConsensusAdapter, Context, ExecutorFactory, ExecutorParams,
    ExecutorResp, Gossip, MemPool, MessageTarget, MixedTxHashes, NodeInfo, Priority, Rpc,
    ServiceMapping, Storage, SynchronizationAdapter,
};
use protocol::types::{
    Address, Epoch, Hash, MerkleRoot, Proof, Receipt, SignedTransaction, Validator,
};
use protocol::ProtocolResult;

use crate::fixed_types::{FixedEpoch, FixedEpochID, FixedSignedTxs, PullTxsRequest};
use crate::message::{BROADCAST_EPOCH_ID, RPC_SYNC_PULL_EPOCH, RPC_SYNC_PULL_TXS};
use crate::status::{StatusAgent, UpdateInfo};
use crate::util::ExecuteInfo;
use crate::ConsensusError;

const OVERLORD_GAP: usize = 10;

pub struct OverlordConsensusAdapter<
    EF: ExecutorFactory<DB, S, Mapping>,
    G: Gossip,
    M: MemPool,
    R: Rpc,
    S: Storage,
    DB: cita_trie::DB,
    Mapping: ServiceMapping,
> {
    rpc:             Arc<R>,
    network:         Arc<G>,
    mempool:         Arc<M>,
    storage:         Arc<S>,
    trie_db:         Arc<DB>,
    service_mapping: Arc<Mapping>,

    exec_queue:  Sender<ExecuteInfo>,
    exec_demons: Option<ExecDemons<S, DB, EF, Mapping>>,
}

#[async_trait]
impl<EF, G, M, R, S, DB, Mapping> ConsensusAdapter
    for OverlordConsensusAdapter<EF, G, M, R, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping>,
    G: Gossip + Sync + Send,
    R: Rpc + Sync + Send,
    M: MemPool,
    S: Storage,
    DB: cita_trie::DB,
    Mapping: ServiceMapping,
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
        node_info: NodeInfo,
        order_root: MerkleRoot,
        epoch_id: u64,
        cycles_price: u64,
        coinbase: Address,
        signed_txs: Vec<SignedTransaction>,
        cycles_limit: u64,
        timestamp: u64,
    ) -> ProtocolResult<()> {
        let chain_id = node_info.chain_id;
        let exec_info = ExecuteInfo {
            epoch_id,
            chain_id,
            cycles_price,
            signed_txs,
            order_root,
            coinbase,
            cycles_limit,
            timestamp,
        };

        let mut tx = self.exec_queue.clone();
        tx.try_send(exec_info)
            .map_err(|e| ConsensusError::ExecuteErr(e.to_string()))?;
        Ok(())
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

#[async_trait]
impl<EF, G, M, R, S, DB, Mapping> SynchronizationAdapter
    for OverlordConsensusAdapter<EF, G, M, R, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping>,
    G: Gossip + Sync + Send,
    R: Rpc + Sync + Send,
    M: MemPool,
    S: Storage,
    DB: cita_trie::DB,
    Mapping: ServiceMapping,
{
    fn sync_exec(
        &self,
        _: Context,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        let mut executor = EF::from_root(
            params.state_root.clone(),
            Arc::clone(&self.trie_db),
            Arc::clone(&self.storage),
            Arc::clone(&self.service_mapping),
        )?;

        let resp = executor.exec(params, txs)?;
        Ok(resp)
    }

    /// Pull some epochs from other nodes from `begin` to `end`.
    async fn get_epoch_from_remote(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        let res = self
            .rpc
            .call::<FixedEpochID, FixedEpoch>(
                ctx,
                RPC_SYNC_PULL_EPOCH,
                FixedEpochID::new(epoch_id),
                Priority::High,
            )
            .await?;
        Ok(res.inner)
    }

    /// Pull signed transactions corresponding to the given hashes from other
    /// nodes.
    async fn get_txs_from_remote(
        &self,
        ctx: Context,
        hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let res = self
            .rpc
            .call::<PullTxsRequest, FixedSignedTxs>(
                ctx,
                RPC_SYNC_PULL_TXS,
                PullTxsRequest::new(hashes.to_vec()),
                Priority::High,
            )
            .await?;
        Ok(res.inner)
    }
}

#[async_trait]
impl<EF, G, M, R, S, DB, Mapping> CommonConsensusAdapter
    for OverlordConsensusAdapter<EF, G, M, R, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping>,
    G: Gossip + Sync + Send,
    R: Rpc + Sync + Send,
    M: MemPool,
    S: Storage,
    DB: cita_trie::DB,
    Mapping: ServiceMapping,
{
    /// Save an epoch to the database.
    async fn save_epoch(&self, _: Context, epoch: Epoch) -> ProtocolResult<()> {
        self.storage.insert_epoch(epoch).await
    }

    async fn save_proof(&self, _: Context, proof: Proof) -> ProtocolResult<()> {
        self.storage.update_latest_proof(proof).await
    }

    /// Save some signed transactions to the database.
    async fn save_signed_txs(
        &self,
        _: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        self.storage.insert_transactions(signed_txs).await
    }

    async fn save_receipts(&self, _: Context, receipts: Vec<Receipt>) -> ProtocolResult<()> {
        self.storage.insert_receipts(receipts).await
    }

    /// Flush the given transactions in the mempool.
    async fn flush_mempool(&self, ctx: Context, ordered_tx_hashes: &[Hash]) -> ProtocolResult<()> {
        self.mempool.flush(ctx, ordered_tx_hashes.to_vec()).await
    }

    /// Get an epoch corresponding to the given epoch ID.
    async fn get_epoch_by_id(&self, _: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        self.storage.get_epoch_by_epoch_id(epoch_id).await
    }

    /// Get the current epoch ID from storage.
    async fn get_current_epoch_id(&self, _: Context) -> ProtocolResult<u64> {
        let res = self.storage.get_latest_epoch().await?;
        Ok(res.header.epoch_id)
    }

    async fn get_txs_from_storage(
        &self,
        ctx: Context,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        self.mempool.get_full_txs(ctx, tx_hashes.to_vec()).await
    }

    async fn broadcast_epoch_id(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<()> {
        self.network
            .broadcast(ctx.clone(), BROADCAST_EPOCH_ID, epoch_id, Priority::High)
            .await
    }
}

impl<EF, G, M, R, S, DB, Mapping> OverlordConsensusAdapter<EF, G, M, R, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping>,
    G: Gossip + Sync + Send,
    R: Rpc + Sync + Send,
    M: MemPool,
    S: Storage,
    DB: cita_trie::DB,
    Mapping: ServiceMapping,
{
    pub fn new(
        rpc: Arc<R>,
        network: Arc<G>,
        mempool: Arc<M>,
        storage: Arc<S>,
        trie_db: Arc<DB>,
        service_mapping: Arc<Mapping>,
        status_agent: StatusAgent,
        state_root: MerkleRoot,
    ) -> Self {
        let (exec_queue, rx) = channel(OVERLORD_GAP);
        let exec_demons = Some(ExecDemons::new(
            Arc::clone(&storage),
            Arc::clone(&trie_db),
            Arc::clone(&service_mapping),
            rx,
            status_agent,
            state_root,
        ));

        OverlordConsensusAdapter {
            rpc,
            network,
            mempool,
            storage,
            trie_db,
            service_mapping,
            exec_queue,
            exec_demons,
        }
    }

    pub fn take_exec_demon(&mut self) -> ExecDemons<S, DB, EF, Mapping> {
        assert!(self.exec_demons.is_some());
        self.exec_demons.take().unwrap()
    }
}

#[derive(Debug)]
pub struct ExecDemons<S, DB, EF, Mapping> {
    storage:         Arc<S>,
    trie_db:         Arc<DB>,
    service_mapping: Arc<Mapping>,

    pin_ef:     PhantomData<EF>,
    queue:      Receiver<ExecuteInfo>,
    state_root: MerkleRoot,
    status:     StatusAgent,
}

impl<S, DB, EF, Mapping> ExecDemons<S, DB, EF, Mapping>
where
    S: Storage,
    DB: cita_trie::DB,
    EF: ExecutorFactory<DB, S, Mapping>,
    Mapping: ServiceMapping,
{
    fn new(
        storage: Arc<S>,
        trie_db: Arc<DB>,
        service_mapping: Arc<Mapping>,
        rx: Receiver<ExecuteInfo>,
        status_agent: StatusAgent,
        state_root: MerkleRoot,
    ) -> Self {
        ExecDemons {
            storage,
            trie_db,
            service_mapping,
            state_root,
            queue: rx,
            pin_ef: PhantomData,
            status: status_agent,
        }
    }

    pub async fn run(mut self) {
        loop {
            if let Err(e) = self.process().await {
                log::error!("muta-consensus: executor demons error {:?}", e);
            }
        }
    }

    async fn process(&mut self) -> ProtocolResult<()> {
        if let Some(info) = self.queue.next().await {
            let epoch_id = info.epoch_id;
            let txs = info.signed_txs.clone();
            let order_root = info.order_root.clone();

            let mut executor = EF::from_root(
                self.state_root.clone(),
                Arc::clone(&self.trie_db),
                Arc::clone(&self.storage),
                Arc::clone(&self.service_mapping),
            )?;
            let exec_params = ExecutorParams {
                state_root: self.state_root.clone(),
                epoch_id,
                timestamp: info.timestamp,
                cycles_limit: info.cycles_limit,
            };
            let resp = executor.exec(&exec_params, &txs)?;
            self.state_root = resp.state_root.clone();
            self.save_receipts(resp.receipts.clone()).await?;
            self.status
                .update_after_exec(UpdateInfo::with_after_exec(epoch_id, order_root, resp));
        } else {
            return Err(ConsensusError::Other("Queue disconnect".to_string()).into());
        }
        Ok(())
    }

    async fn save_receipts(&self, receipts: Vec<Receipt>) -> ProtocolResult<()> {
        self.storage.insert_receipts(receipts).await
    }
}
