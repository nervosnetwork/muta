use async_trait::async_trait;
use creep::Context;

use crate::traits::{ExecutorParams, ExecutorResp, TrustFeedback};
use crate::types::{
    Address, Block, Bytes, Hash, MerkleRoot, Metadata, Proof, Receipt, SignedTransaction, Validator,
};
use crate::{traits::mempool::MixedTxHashes, ProtocolResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageTarget {
    Broadcast,
    Specified(Address),
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub chain_id:     Hash,
    pub self_address: Address,
}

#[async_trait]
pub trait Consensus: Send + Sync {
    /// Network set a received signed proposal to consensus.
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()>;

    /// Network set a received signed vote to consensus.
    async fn set_vote(&self, ctx: Context, vote: Vec<u8>) -> ProtocolResult<()>;

    /// Network set a received quorum certificate to consensus.
    async fn set_qc(&self, ctx: Context, qc: Vec<u8>) -> ProtocolResult<()>;

    /// Network set a received signed choke to consensus.
    async fn set_choke(&self, ctx: Context, choke: Vec<u8>) -> ProtocolResult<()>;
}

#[async_trait]
pub trait Synchronization: Send + Sync {
    async fn receive_remote_block(&self, ctx: Context, remote_height: u64) -> ProtocolResult<()>;
}

#[async_trait]
pub trait SynchronizationAdapter: CommonConsensusAdapter + Send + Sync {
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
    ) -> ProtocolResult<()>;

    fn sync_exec(
        &self,
        ctx: Context,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp>;

    /// Pull some blocks from other nodes from `begin` to `end`.
    async fn get_block_from_remote(&self, ctx: Context, height: u64) -> ProtocolResult<Block>;

    /// Pull signed transactions corresponding to the given hashes from other
    /// nodes.
    async fn get_txs_from_remote(
        &self,
        ctx: Context,
        hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>>;
}

#[async_trait]
pub trait CommonConsensusAdapter: Send + Sync {
    /// Save a block to the database.
    async fn save_block(&self, ctx: Context, block: Block) -> ProtocolResult<()>;

    async fn save_proof(&self, ctx: Context, proof: Proof) -> ProtocolResult<()>;

    /// Save some signed transactions to the database.
    async fn save_signed_txs(
        &self,
        ctx: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()>;

    async fn save_receipts(&self, ctx: Context, receipts: Vec<Receipt>) -> ProtocolResult<()>;

    /// Flush the given transactions in the mempool.
    async fn flush_mempool(&self, ctx: Context, ordered_tx_hashes: &[Hash]) -> ProtocolResult<()>;

    /// Get a block corresponding to the given height.
    async fn get_block_by_height(&self, ctx: Context, height: u64) -> ProtocolResult<Block>;

    /// Get the current height from storage.
    async fn get_current_height(&self, ctx: Context) -> ProtocolResult<u64>;

    async fn get_txs_from_storage(
        &self,
        ctx: Context,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    async fn broadcast_height(&self, ctx: Context, height: u64) -> ProtocolResult<()>;

    /// Get metadata by the giving state_root.
    fn get_metadata(
        &self,
        context: Context,
        state_root: MerkleRoot,
        height: u64,
        timestamp: u64,
    ) -> ProtocolResult<Metadata>;

    fn report_bad(&self, ctx: Context, feedback: TrustFeedback);

    fn set_args(&self, context: Context, timeout_gap: u64, cycles_limit: u64, max_tx_size: u64);
}

#[async_trait]
pub trait ConsensusAdapter: CommonConsensusAdapter + Send + Sync {
    /// Get some transaction hashes of the given height. The amount of the
    /// transactions is limited by the given cycle limit and return a
    /// `MixedTxHashes` struct.
    async fn get_txs_from_mempool(
        &self,
        ctx: Context,
        height: u64,
        cycle_limit: u64,
        tx_num_limit: u64,
    ) -> ProtocolResult<MixedTxHashes>;

    /// Check the correctness of the given transactions.
    async fn check_txs(&self, ctx: Context, order_txs: Vec<Hash>) -> ProtocolResult<()>;

    /// Synchronous signed transactions.
    async fn sync_txs(&self, ctx: Context, propose_txs: Vec<Hash>) -> ProtocolResult<()>;

    /// Get the signed transactions corresponding to the given hashes.
    async fn get_full_txs(
        &self,
        ctx: Context,
        order_txs: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    /// Consensus transmit a message to the given target.
    async fn transmit(
        &self,
        ctx: Context,
        msg: Vec<u8>,
        end: &str,
        target: MessageTarget,
    ) -> ProtocolResult<()>;

    /// Execute some transactions.
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
    ) -> ProtocolResult<()>;

    /// Get the validator list of the given last block.
    async fn get_last_validators(
        &self,
        ctx: Context,
        height: u64,
    ) -> ProtocolResult<Vec<Validator>>;

    /// Get the current height from storage.
    async fn get_current_height(&self, ctx: Context) -> ProtocolResult<u64>;

    /// Pull some blocks from other nodes from `begin` to `end`.
    async fn pull_block(&self, ctx: Context, height: u64, end: &str) -> ProtocolResult<Block>;

    /// Pull signed transactions corresponding to the given hashes from other
    /// nodes.
    async fn pull_txs(
        &self,
        ctx: Context,
        hashes: Vec<Hash>,
        end: &str,
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    /// Save overlord wal info.
    async fn save_overlord_wal(&self, ctx: Context, info: Bytes) -> ProtocolResult<()>;

    /// Load latest overlord wal info.
    async fn load_overlord_wal(&self, ctx: Context) -> ProtocolResult<Bytes>;
}
