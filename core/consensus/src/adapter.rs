use std::boxed::Box;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::stream::StreamExt;
use overlord::types::OverlordMsg;
use overlord::OverlordHandler;
use parking_lot::RwLock;

use common_merkle::Merkle;
use protocol::traits::{
    CommonConsensusAdapter, ConsensusAdapter, Context, ExecutorFactory, ExecutorParams,
    ExecutorResp, Gossip, MemPool, MessageTarget, MixedTxHashes, Priority, Rpc, ServiceMapping,
    Storage, SynchronizationAdapter,
};
use protocol::types::{
    Address, Block, Bytes, Hash, MerkleRoot, Metadata, Proof, Receipt, SignedTransaction,
    TransactionRequest, Validator,
};
use protocol::{fixed_codec::FixedCodec, ProtocolResult};

use crate::consensus::gen_overlord_status;
use crate::fixed_types::{FixedBlock, FixedHeight, FixedPill, FixedSignedTxs, PullTxsRequest};
use crate::message::{BROADCAST_HEIGHT, RPC_SYNC_PULL_BLOCK, RPC_SYNC_PULL_TXS};
use crate::status::{ExecutedInfo, StatusAgent};
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
    rpc:              Arc<R>,
    network:          Arc<G>,
    mempool:          Arc<M>,
    storage:          Arc<S>,
    trie_db:          Arc<DB>,
    service_mapping:  Arc<Mapping>,
    overlord_handler: RwLock<Option<OverlordHandler<FixedPill>>>,

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
    M: MemPool + 'static,
    S: Storage + 'static,
    DB: cita_trie::DB + 'static,
    Mapping: ServiceMapping + 'static,
{
    async fn get_txs_from_mempool(
        &self,
        ctx: Context,
        _height: u64,
        cycle_limit: u64,
        tx_num_limit: u64,
    ) -> ProtocolResult<MixedTxHashes> {
        self.mempool.package(ctx, cycle_limit, tx_num_limit).await
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
        chain_id: Hash,
        order_root: MerkleRoot,
        height: u64,
        cycles_price: u64,
        coinbase: Address,
        block_hash: Hash,
        signed_txs: Vec<SignedTransaction>,
        cycles_limit: u64,
        timestamp: u64,
    ) -> ProtocolResult<()> {
        let exec_info = ExecuteInfo {
            height,
            chain_id,
            cycles_price,
            block_hash,
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
        height: u64,
    ) -> ProtocolResult<Vec<Validator>> {
        let block = self.storage.get_block_by_height(height).await?;
        Ok(block.header.validators)
    }

    async fn save_overlord_wal(&self, _ctx: Context, info: Bytes) -> ProtocolResult<()> {
        self.storage.update_overlord_wal(info).await
    }

    async fn load_overlord_wal(&self, _ctx: Context) -> ProtocolResult<Bytes> {
        self.storage.load_overlord_wal().await
    }

    async fn pull_block(&self, ctx: Context, height: u64, end: &str) -> ProtocolResult<Block> {
        log::debug!("consensus: send rpc pull block {}", height);
        let res = self
            .rpc
            .call::<FixedHeight, FixedBlock>(ctx, end, FixedHeight::new(height), Priority::High)
            .await?;
        Ok(res.inner)
    }

    async fn pull_txs(
        &self,
        ctx: Context,
        hashes: Vec<Hash>,
        end: &str,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        log::debug!("consensus: send rpc pull txs");
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

    /// Get the current height from storage.
    async fn get_current_height(&self, _: Context) -> ProtocolResult<u64> {
        let res = self.storage.get_latest_block().await?;
        Ok(res.header.height)
    }
}

#[async_trait]
impl<EF, G, M, R, S, DB, Mapping> SynchronizationAdapter
    for OverlordConsensusAdapter<EF, G, M, R, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping>,
    G: Gossip + Sync + Send,
    R: Rpc + Sync + Send,
    M: MemPool + 'static,
    S: Storage + 'static,
    DB: cita_trie::DB + 'static,
    Mapping: ServiceMapping + 'static,
{
    fn update_status(
        &self,
        ctx: Context,
        height: u64,
        consensus_interval: u64,
        propose_ratio: u64,
        prevote_ratio: u64,
        precommit_ratio: u64,
        brake_ratio: u64,
        validators: Vec<Validator>,
    ) -> ProtocolResult<()> {
        self.overlord_handler
            .read()
            .as_ref()
            .expect("Please set the overlord handle first")
            .send_msg(
                ctx,
                OverlordMsg::RichStatus(gen_overlord_status(
                    height + 1,
                    consensus_interval,
                    propose_ratio,
                    prevote_ratio,
                    precommit_ratio,
                    brake_ratio,
                    validators,
                )),
            )
            .map_err(|e| ConsensusError::OverlordErr(Box::new(e)))?;
        Ok(())
    }

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

    /// Pull some blocks from other nodes from `begin` to `end`.
    async fn get_block_from_remote(&self, ctx: Context, height: u64) -> ProtocolResult<Block> {
        let res = self
            .rpc
            .call::<FixedHeight, FixedBlock>(
                ctx,
                RPC_SYNC_PULL_BLOCK,
                FixedHeight::new(height),
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
    M: MemPool + 'static,
    S: Storage + 'static,
    DB: cita_trie::DB + 'static,
    Mapping: ServiceMapping + 'static,
{
    /// Save a block to the database.
    async fn save_block(&self, _: Context, block: Block) -> ProtocolResult<()> {
        self.storage.insert_block(block).await
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

    /// Get a block corresponding to the given height.
    async fn get_block_by_height(&self, _: Context, height: u64) -> ProtocolResult<Block> {
        self.storage.get_block_by_height(height).await
    }

    /// Get the current height from storage.
    async fn get_current_height(&self, _: Context) -> ProtocolResult<u64> {
        let res = self.storage.get_latest_block().await?;
        Ok(res.header.height)
    }

    async fn get_txs_from_storage(
        &self,
        _: Context,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        self.storage.get_transactions(tx_hashes.to_vec()).await
    }

    async fn broadcast_height(&self, ctx: Context, height: u64) -> ProtocolResult<()> {
        self.network
            .broadcast(ctx.clone(), BROADCAST_HEIGHT, height, Priority::High)
            .await
    }

    /// Get metadata by the giving height.
    fn get_metadata(
        &self,
        _: Context,
        state_root: MerkleRoot,
        height: u64,
        timestamp: u64,
    ) -> ProtocolResult<Metadata> {
        let executor = EF::from_root(
            state_root.clone(),
            Arc::clone(&self.trie_db),
            Arc::clone(&self.storage),
            Arc::clone(&self.service_mapping),
        )?;

        let caller = Address::from_hex("0x0000000000000000000000000000000000000000")?;

        let params = ExecutorParams {
            state_root,
            height,
            timestamp,
            cycles_limit: u64::max_value(),
        };
        let exec_resp = executor.read(&params, &caller, 1, &TransactionRequest {
            service_name: "metadata".to_string(),
            method:       "get_metadata".to_string(),
            payload:      "".to_string(),
        })?;

        Ok(serde_json::from_str(&exec_resp.ret).expect("Decode metadata failed!"))
    }

    fn set_args(&self, _context: Context, timeout_gap: u64, cycles_limit: u64, max_tx_size: u64) {
        self.mempool
            .set_args(timeout_gap, cycles_limit, max_tx_size);
    }
}

impl<EF, G, M, R, S, DB, Mapping> OverlordConsensusAdapter<EF, G, M, R, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping>,
    G: Gossip + Sync + Send,
    R: Rpc + Sync + Send,
    M: MemPool + 'static,
    S: Storage + 'static,
    DB: cita_trie::DB + 'static,
    Mapping: ServiceMapping + 'static,
{
    pub fn new(
        rpc: Arc<R>,
        network: Arc<G>,
        mempool: Arc<M>,
        storage: Arc<S>,
        trie_db: Arc<DB>,
        service_mapping: Arc<Mapping>,
        status_agent: StatusAgent,
    ) -> ProtocolResult<Self> {
        let (exec_queue, rx) = channel(OVERLORD_GAP);
        let exec_demons = Some(ExecDemons::new(
            Arc::clone(&storage),
            Arc::clone(&trie_db),
            Arc::clone(&service_mapping),
            rx,
            status_agent,
        ));

        let adapter = OverlordConsensusAdapter {
            rpc,
            network,
            mempool,
            storage,
            trie_db,
            service_mapping,
            overlord_handler: RwLock::new(None),
            exec_queue,
            exec_demons,
        };

        Ok(adapter)
    }

    pub fn take_exec_demon(&mut self) -> ExecDemons<S, DB, EF, Mapping> {
        assert!(self.exec_demons.is_some());
        self.exec_demons.take().unwrap()
    }

    pub fn set_overlord_handler(&self, handler: OverlordHandler<FixedPill>) {
        *self.overlord_handler.write() = Some(handler)
    }
}

#[derive(Debug)]
pub struct ExecDemons<S, DB, EF, Mapping> {
    storage:         Arc<S>,
    trie_db:         Arc<DB>,
    service_mapping: Arc<Mapping>,

    pin_ef: PhantomData<EF>,
    queue:  Receiver<ExecuteInfo>,
    status: StatusAgent,
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
    ) -> Self {
        ExecDemons {
            storage,
            trie_db,
            service_mapping,
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
            let height = info.height;
            let txs = info.signed_txs.clone();
            let order_root = info.order_root.clone();
            let state_root = self.status.to_inner().get_latest_state_root();

            let now = Instant::now();
            let mut executor = EF::from_root(
                state_root.clone(),
                Arc::clone(&self.trie_db),
                Arc::clone(&self.storage),
                Arc::clone(&self.service_mapping),
            )?;
            let exec_params = ExecutorParams {
                state_root: state_root.clone(),
                height,
                timestamp: info.timestamp,
                cycles_limit: info.cycles_limit,
            };
            let resp = executor.exec(&exec_params, &txs)?;
            log::info!(
                "[consensus-adapter]: exec transactions cost {:?} transactions len {:?}",
                now.elapsed(),
                txs.len(),
            );

            let now = Instant::now();
            self.save_receipts(resp.receipts.clone()).await?;
            log::info!(
                "[consensus-adapter]: save receipts cost {:?} receipts len {:?}",
                now.elapsed(),
                resp.receipts.len(),
            );
            self.status
                .update_by_executed(gen_executed_info(resp.clone(), height, order_root));
        } else {
            return Err(ConsensusError::Other("Queue disconnect".to_string()).into());
        }
        Ok(())
    }

    async fn save_receipts(&self, receipts: Vec<Receipt>) -> ProtocolResult<()> {
        self.storage.insert_receipts(receipts).await
    }
}

fn gen_executed_info(exec_resp: ExecutorResp, height: u64, order_root: MerkleRoot) -> ExecutedInfo {
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

    ExecutedInfo {
        exec_height:  height,
        cycles_used:  cycles,
        receipt_root: receipt,
        confirm_root: order_root,
        state_root:   exec_resp.state_root.clone(),
        logs_bloom:   exec_resp.logs_bloom,
    }
}
