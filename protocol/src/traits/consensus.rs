use async_trait::async_trait;
use creep::Context;

use crate::types::{Epoch, Hash, Proof, Receipt, SignedTransaction, UserAddress, Validator};
use crate::{traits::mempool::MixedTxHashes, ProtocolResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageTarget {
    Broadcast,
    Specified(UserAddress),
}

#[async_trait]
pub trait Consensus<Adapter: ConsensusAdapter>: Send + Sync {
    /// Network set a received signed proposal to consensus.
    async fn set_proposal(&self, ctx: Context, proposal: Vec<u8>) -> ProtocolResult<()>;

    /// Network set a received signed vote to consensus.
    async fn set_vote(&self, ctx: Context, vote: Vec<u8>) -> ProtocolResult<()>;

    /// Network set a received quorum certificate to consensus.
    async fn set_qc(&self, ctx: Context, qc: Vec<u8>) -> ProtocolResult<()>;

    /// Update an epoch to consensus. This may be either a rich status from the
    /// executor or a synchronous epoch that need to be insert to the database.
    async fn update_epoch(
        &self,
        ctx: Context,
        epoch: Epoch,
        signed_txs: Vec<SignedTransaction>,
        proof: Proof,
    ) -> ProtocolResult<()>;
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
    async fn execute(&self, ctx: Context, signed_txs: Vec<SignedTransaction>)
        -> ProtocolResult<()>;

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
}
