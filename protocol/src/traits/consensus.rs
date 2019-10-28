use async_trait::async_trait;
use creep::Context;

use crate::types::{
    Address, Bloom, Epoch, Hash, MerkleRoot, Proof, Receipt, SignedTransaction, UserAddress,
    Validator,
};
use crate::{traits::executor::ExecutorExecResp, traits::mempool::MixedTxHashes, ProtocolResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageTarget {
    Broadcast,
    Specified(UserAddress),
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub chain_id:     Hash,
    pub self_address: UserAddress,
}

#[derive(Clone, Debug)]
pub struct CurrentConsensusStatus {
    pub cycles_price:       u64,
    pub cycles_limit:       u64,
    pub epoch_id:           u64,
    pub prev_hash:          Hash,
    pub logs_bloom:         Bloom,
    pub order_root:         MerkleRoot,
    pub confirm_root:       Vec<MerkleRoot>,
    pub state_root:         MerkleRoot,
    pub receipt_root:       Vec<MerkleRoot>,
    pub cycles_used:        u64,
    pub proof:              Proof,
    pub validators:         Vec<Validator>,
    pub consensus_interval: u64,
}

#[async_trait]
pub trait Consensus: Send + Sync {
    /// Network set a received signed proposal to consensus.
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()>;

    /// Network set a received signed vote to consensus.
    async fn set_vote(&self, ctx: Context, vote: Vec<u8>) -> ProtocolResult<()>;

    /// Network set a received quorum certificate to consensus.
    async fn set_qc(&self, ctx: Context, qc: Vec<u8>) -> ProtocolResult<()>;

    /// Update an epoch to consensus. This may be either a rich status from the
    /// executor or a synchronous epoch that need to be insert to the database.
    async fn update_epoch(&self, ctx: Context, msg: Vec<u8>) -> ProtocolResult<()>;
}

#[async_trait]
pub trait ConsensusAdapter: Send + Sync {
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
    async fn check_txs(&self, ctx: Context, txs: Vec<Hash>) -> ProtocolResult<()>;

    /// Synchronous signed transactions.
    async fn sync_txs(&self, ctx: Context, txs: Vec<Hash>) -> ProtocolResult<()>;

    /// Get the signed transactions corresponding to the given hashes.
    async fn get_full_txs(
        &self,
        ctx: Context,
        txs: Vec<Hash>,
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
        state_root: MerkleRoot,
        epoch_id: u64,
        cycles_price: u64,
        coinbase: Address,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<ExecutorExecResp>;

    /// Flush the given transactions in the mempool.
    async fn flush_mempool(&self, ctx: Context, txs: Vec<Hash>) -> ProtocolResult<()>;

    /// Save an epoch to the database.
    async fn save_epoch(&self, ctx: Context, epoch: Epoch) -> ProtocolResult<()>;

    /// Save some receipts to the database.
    async fn save_receipts(&self, ctx: Context, receipts: Vec<Receipt>) -> ProtocolResult<()>;

    ///
    async fn save_proof(&self, ctx: Context, proof: Proof) -> ProtocolResult<()>;

    /// Save some signed transactions to the database.
    async fn save_signed_txs(
        &self,
        ctx: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()>;

    /// Get the validator list of the given last epoch.
    async fn get_last_validators(
        &self,
        ctx: Context,
        epoch_id: u64,
    ) -> ProtocolResult<Vec<Validator>>;

    /// Get the current epoch ID from storage.
    async fn get_current_epoch_id(&self, ctx: Context) -> ProtocolResult<u64>;

    /// Pull some epochs from other nodes from `begin` to `end`.
    async fn pull_epoch(&self, ctx: Context, epoch_id: u64, end: &str) -> ProtocolResult<Epoch>;

    /// Pull signed transactions corresponding to the given hashes from other
    /// nodes.
    async fn pull_txs(
        &self,
        ctx: Context,
        hashes: Vec<Hash>,
        end: &str,
    ) -> ProtocolResult<Vec<SignedTransaction>>;

    /// Get an epoch corresponding to the given epoch ID.
    async fn get_epoch_by_id(&self, ctx: Context, epoch_id: u64) -> ProtocolResult<Epoch>;
}
