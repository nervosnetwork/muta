use std::boxed::Box;
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::stream::StreamExt;
use log::{debug, error};

use common_merkle::Merkle;
use protocol::traits::{
    ConsensusAdapter, Context, ExecutorFactory, ExecutorParams, ExecutorResp, Gossip, MemPool,
    MessageTarget, MixedTxHashes, NodeInfo, Priority, Rpc, Storage,
};
use protocol::types::{
    Address, Epoch, Hash, MerkleRoot, Proof, Receipt, SignedTransaction, Validator,
};
use protocol::{fixed_codec::FixedCodec, ProtocolResult};

use crate::fixed_types::{FixedEpoch, FixedEpochID, FixedSignedTxs, PullTxsRequest};
use crate::status::{CurrentStatusAgent, UpdateInfo};
use crate::util::ExecuteInfo;
use crate::ConsensusError;

const OVERLORD_GAP: usize = 10;

pub struct OverlordConsensusAdapter<
    EF: ExecutorFactory<DB, S>,
    G: Gossip,
    M: MemPool,
    R: Rpc,
    S: Storage,
    DB: cita_trie::DB,
> {
    rpc:     Arc<R>,
    network: Arc<G>,
    mempool: Arc<M>,
    storage: Arc<S>,

    exec_queue:  Sender<ExecuteInfo>,
    exec_demons: Option<ExecDemons<S, DB, EF>>,
}

#[async_trait]
impl<EF, G, M, R, S, DB> ConsensusAdapter for OverlordConsensusAdapter<EF, G, M, R, S, DB>
where
    EF: ExecutorFactory<DB, S>,
    G: Gossip + Sync + Send,
    R: Rpc + Sync + Send,
    M: MemPool,
    S: Storage,
    DB: cita_trie::DB,
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

    async fn flush_mempool(&self, ctx: Context, txs: Vec<Hash>) -> ProtocolResult<()> {
        self.mempool.flush(ctx, txs).await
    }

    async fn save_epoch(&self, _ctx: Context, epoch: Epoch) -> ProtocolResult<()> {
        self.storage.insert_epoch(epoch).await
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

    async fn get_current_epoch_id(&self, _ctx: Context) -> ProtocolResult<u64> {
        let res = self.storage.get_latest_epoch().await?;
        Ok(res.header.epoch_id)
    }

    async fn pull_epoch(&self, ctx: Context, epoch_id: u64, end: &str) -> ProtocolResult<Epoch> {
        debug!("consensus: send rpc pull epoch {}", epoch_id);
        let res = self
            .rpc
            .call::<FixedEpochID, FixedEpoch>(ctx, end, FixedEpochID::new(epoch_id), Priority::High)
            .await?;
        Ok(res.inner)
    }

    async fn pull_txs(
        &self,
        ctx: Context,
        hashes: Vec<Hash>,
        end: &str,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        debug!("consensus: send rpc pull txs");
        let res = self
            .rpc
            .call::<PullTxsRequest, FixedSignedTxs>(
                ctx,
                end,
                PullTxsRequest::new(hashes),
                Priority::High,
            )
            .await?;
        Ok(res.inner)
    }

    async fn get_epoch_by_id(&self, _ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        self.storage.get_epoch_by_epoch_id(epoch_id).await
    }
}

impl<EF, G, M, R, S, DB> OverlordConsensusAdapter<EF, G, M, R, S, DB>
where
    EF: ExecutorFactory<DB, S>,
    G: Gossip + Sync + Send,
    R: Rpc + Sync + Send,
    M: MemPool,
    S: Storage,
    DB: cita_trie::DB,
{
    pub fn new(
        rpc: Arc<R>,
        network: Arc<G>,
        mempool: Arc<M>,
        storage: Arc<S>,
        trie_db: Arc<DB>,
        status_agent: CurrentStatusAgent,
        state_root: MerkleRoot,
    ) -> Self {
        let (exec_queue, rx) = channel(OVERLORD_GAP);
        let exec_demons = Some(ExecDemons::new(
            Arc::clone(&storage),
            trie_db,
            rx,
            status_agent,
            state_root,
        ));

        OverlordConsensusAdapter {
            rpc,
            network,
            mempool,
            storage,
            exec_queue,
            exec_demons,
        }
    }

    pub fn take_exec_demon(&mut self) -> ExecDemons<S, DB, EF> {
        assert!(self.exec_demons.is_some());
        self.exec_demons.take().unwrap()
    }
}

#[derive(Debug)]
pub struct ExecDemons<S, DB, EF> {
    storage: Arc<S>,
    trie_db: Arc<DB>,

    pin_ef:     PhantomData<EF>,
    queue:      Receiver<ExecuteInfo>,
    state_root: MerkleRoot,
    status:     CurrentStatusAgent,
}

impl<S, DB, EF> ExecDemons<S, DB, EF>
where
    S: Storage,
    DB: cita_trie::DB,
    EF: ExecutorFactory<DB, S>,
{
    fn new(
        storage: Arc<S>,
        trie_db: Arc<DB>,
        rx: Receiver<ExecuteInfo>,
        status_agent: CurrentStatusAgent,
        state_root: MerkleRoot,
    ) -> Self {
        ExecDemons {
            storage,
            trie_db,
            state_root,
            queue: rx,
            pin_ef: PhantomData,
            status: status_agent,
        }
        Ok(())
    }

    async fn save_receipts(&self, receipts: Vec<Receipt>) -> ProtocolResult<()> {
        self.storage.insert_receipts(receipts).await
    }
}

fn gen_update_info(
    exec_resp: ExecutorExecResp,
    epoch_id: u64,
    order_root: MerkleRoot,
) -> UpdateInfo {
    let cycles = exec_resp.all_cycles_used.iter().map(|fee| fee.cycle).sum();
    let receipt = Merkle::from_hashes(
        exec_resp
            .receipts
            .iter()
            .map(|r| Hash::digest(r.to_owned().encode_fixed().unwrap()))
            .collect::<Vec<_>>(),
    )
    .get_root_hash()
    .unwrap_or_else(Hash::from_empty);

    UpdateInfo {
        exec_epoch_id: epoch_id,
        cycles_used:   cycles,
        receipt_root:  receipt,
        confirm_root:  order_root,
        state_root:    exec_resp.state_root.clone(),
        logs_bloom:    exec_resp.logs_bloom,
    }

    pub async fn run(mut self) {
        loop {
            if let Err(e) = self.process().await {
                error!("muta-consensus: executor demons error {:?}", e);
            }
        }
    }

    async fn process(&mut self) -> ProtocolResult<()> {
        if let Some(info) = self.queue.next().await {
            let epoch_id = info.epoch_id;
            let txs = info.signed_txs.clone();
            let order_root = info.order_root.clone();

            error!("muta-consensus: execute {} epoch", epoch_id);
            let mut executor = EF::from_root(
                self.state_root.clone(),
                Arc::clone(&self.trie_db),
                Arc::clone(&self.storage),
            )?;
            let exec_params = ExecutorParams {
                state_root: self.state_root.clone(),
                epoch_id,
                timestamp: info.timestamp,
                cycels_limit: info.cycles_limit,
            };
            let resp = executor.exec(&exec_params, &txs)?;
            self.state_root = resp.state_root.clone();
            self.save_receipts(resp.receipts.clone()).await?;
            self.status
                .send(gen_update_info(resp.clone(), epoch_id, order_root))?;
        } else {
            return Err(ConsensusError::Other("Queue disconnect".to_string()).into());
        }
        Ok(())
    }

    async fn save_receipts(&self, receipts: Vec<Receipt>) -> ProtocolResult<()> {
        self.storage.insert_receipts(receipts).await
    }
}

fn gen_update_info(exec_resp: ExecutorResp, epoch_id: u64, order_root: MerkleRoot) -> UpdateInfo {
    let cycles = exec_resp.all_cycles_used;

    let receipt = Merkle::from_hashes(
        exec_resp
            .receipts
            .iter()
            .map(|r| Hash::digest(r.to_owned().encode_fixed().unwrap()))
            .collect::<Vec<_>>(),
    )
    .get_root_hash()
    .unwrap_or_else(Hash::from_empty);

    UpdateInfo {
        exec_epoch_id: epoch_id,
        cycles_used:   cycles,
        receipt_root:  receipt,
        confirm_root:  order_root,
        state_root:    exec_resp.state_root.clone(),
        logs_bloom:    exec_resp.logs_bloom,
    }
}
