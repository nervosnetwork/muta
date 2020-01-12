use async_trait::async_trait;
use creep::Context;

use crate::traits::{ExecutorParams, ExecutorResp};
use crate::types::{
    Address, Epoch, Hash, MerkleRoot, Proof, Receipt, SignedTransaction, Validator,
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
}

#[async_trait]
pub trait Synchronization: Send + Sync {
    async fn receive_remote_epoch(&self, ctx: Context, remote_epoch_id: u64) -> ProtocolResult<()>;
}

#[async_trait]
pub trait SynchronizationAdapter: CommonConsensusAdapter + Send + Sync {
    fn sync_exec(
        &self,
        ctx: Context,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp>;

    /// Pull some epochs from other nodes from `begin` to `end`.
    async fn get_epoch_from_remote(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch>;

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
    /// Save an epoch to the database.
    async fn save_epoch(&self, ctx: Context, epoch: Epoch) -> ProtocolResult<()>;

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

    /// Get an epoch corresponding to the given epoch ID.
    async fn get_epoch_by_id(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch>;

    /// Get the current epoch ID from storage.
    async fn get_current_epoch_id(&self, ctx: Context) -> ProtocolResult<u64>;

    async fn get_txs_from_storage(
        &self,
        ctx: Context,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    async fn broadcast_epoch_id(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<()>;
}

#[async_trait]
pub trait ConsensusAdapter: CommonConsensusAdapter + Send + Sync {
    /// Get some transaction hashes of the given epoch ID. The amount of the
    /// transactions is limited by the given cycle limit and return a
    /// `MixedTxHashes` struct.
    async fn get_txs_from_mempool(
        &self,
        ctx: Context,
        epoch_id: u64,
        cycle_limit: u64,
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
        node_info: NodeInfo,
        order_root: MerkleRoot,
        epoch_id: u64,
        cycles_price: u64,
        coinbase: Address,
        signed_txs: Vec<SignedTransaction>,
        cycles_limit: u64,
        timestamp: u64,
    ) -> ProtocolResult<()>;

    /// Get the validator list of the given last epoch.
    async fn get_last_validators(
        &self,
        ctx: Context,
        epoch_id: u64,
    ) -> ProtocolResult<Vec<Validator>>;
}
