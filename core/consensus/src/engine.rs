use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::{
    compat::Future01CompatExt,
    stream::{FuturesUnordered, TryStreamExt},
};

use core_context::Context;
use core_crypto::Crypto;
use core_merkle::Merkle;
use core_runtime::{ExecutionContext, ExecutionResult, Executor, TransactionPool};
use core_storage::Storage;
use core_types::{
    Address, Block, BlockHeader, Hash, Proof, Proposal, SignedTransaction, TransactionPosition,
};

use crate::{ConsensusError, ConsensusResult, ConsensusStatus};

/// The "Engine" contains the logic required for all consensus except voting.
///
/// If this node is a proposer.
/// step:
/// 1. Get a batch of transactions from the transaction pool and package them
/// into "proposal", call "build_proposal". 2. If the consensus condition is
/// met, execute and submit the "Proposal", call "commit_block".
///
/// If this node is not a "proposer".
/// step:
/// 1. Verify proposal from other nodes, call "verify_proposal".
/// 2. Verify that the transactions in the proposal has a transaction pool for
/// that node, call "verify_transactions". If it does not exist, the transaction
/// pool will actively pull the transactions from the proposed node. If the pull
/// fails, the verification will fail. 3. If the consensus condition is met,
/// execute and submit the "Proposal", call "commit_block".
#[derive(Debug)]
pub struct Engine<E, T, S, C>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
{
    executor: Arc<E>,
    tx_pool:  Arc<T>,
    storage:  Arc<S>,
    crypto:   Arc<C>,

    privkey: C::PrivateKey,
    status:  RwLock<ConsensusStatus>,
}

impl<E, T, S, C> Engine<E, T, S, C>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
{
    pub fn new(
        executor: Arc<E>,
        tx_pool: Arc<T>,
        storage: Arc<S>,
        crypto: Arc<C>,

        privkey: C::PrivateKey,
        status: ConsensusStatus,
    ) -> Self {
        Self {
            executor,
            tx_pool,
            storage,
            crypto,

            privkey,
            status: RwLock::new(status),
        }
    }

    /// Package a new block.
    pub(crate) async fn build_proposal(&self, ctx: Context) -> ConsensusResult<Proposal> {
        let status = self.get_status()?;
        let tx_hashes = await!(self
            .tx_pool
            .package(ctx.clone(), status.tx_limit, status.quota_limit)
            .compat())?;

        let proposal = Proposal {
            timestamp: time_now(),
            prevhash: status.block_hash.clone(),
            height: status.height + 1,
            quota_limit: status.quota_limit,
            proposer: status.node_address.clone(),
            proof: status.proof,
            tx_hashes,
        };
        log::info!(target: "engine", "build proposal {:?}", proposal);
        Ok(proposal)
    }

    // Verify signature of proposal.
    pub(crate) fn verify_signature(
        &self,
        hash: &Hash,
        signature: &C::Signature,
    ) -> ConsensusResult<Address> {
        let pubkey = self.crypto.verify_with_signature(&hash, &signature)?;
        Ok(self.crypto.pubkey_to_address(&pubkey))
    }

    // Sign the proposal hash.
    pub(crate) fn sign_with_hash(&self, hash: &Hash) -> ConsensusResult<C::Signature> {
        let signature = self.crypto.sign(&hash, &self.privkey)?;
        Ok(signature)
    }

    /// Verify proposal block
    pub(crate) fn verify_proposal(&self, _: Context, proposal: &Proposal) -> ConsensusResult<()> {
        log::debug!("verify proposal {:?}", proposal);

        let status = self.get_status()?;

        // check height
        if proposal.height != status.height + 1 {
            return Err(ConsensusError::InvalidProposal("invalid height".to_owned()));
        }
        // check timestamp
        if !check_timestamp(proposal.timestamp, status.timestamp, status.interval) {
            // Ignore the first block after the genesis block.
            if proposal.height != 1 {
                return Err(ConsensusError::InvalidProposal(
                    "invalid timestamp".to_owned(),
                ));
            }
        }
        // check quota limit
        if proposal.quota_limit != status.quota_limit {
            return Err(ConsensusError::InvalidProposal(
                "invalid quota limit".to_owned(),
            ));
        }
        // check prevhash
        if proposal.prevhash != status.block_hash {
            return Err(ConsensusError::InvalidProposal(
                "invalid prevhash".to_owned(),
            ));
        }
        Ok(())
    }

    /// Verify proposal transactions
    pub(crate) async fn verify_transactions(
        &self,
        ctx: Context,
        proposal: Proposal,
    ) -> ConsensusResult<()> {
        log::debug!("verify transactions {:?}", proposal);
        await!(self
            .tx_pool
            .ensure(ctx.clone(), &proposal.tx_hashes)
            .compat())?;
        Ok(())
    }

    /// Commit a block of consensus completion.
    /// step:
    /// 1. Get the transactions contained in the block from the transaction
    /// pool. 2. Execute all transactions with "executor".
    /// 3. build block
    /// 4. save block
    /// 5. flush transaction pool
    /// 6. update status
    pub(crate) async fn commit_block(
        &self,
        ctx: Context,
        proposal: Proposal,
        latest_proof: Proof,
    ) -> ConsensusResult<ConsensusStatus> {
        let status = self.get_status()?;

        // Get transactions from the transaction pool
        let signed_txs = await!(self
            .tx_pool
            .get_batch(ctx.clone(), &proposal.tx_hashes)
            .compat())?;

        // exec transactions
        let execution_context = ExecutionContext {
            state_root:  status.state_root.clone(),
            proposer:    proposal.proposer.clone(),
            height:      proposal.height,
            quota_limit: proposal.quota_limit,
            timestamp:   proposal.timestamp,
        };
        let execution_result = self
            .executor
            .exec(ctx.clone(), &execution_context, &signed_txs)?;

        // build block
        let block = build_block(&proposal, &execution_result);

        // save
        let block_hash = block.hash.clone();
        let cloned_header = block.header.clone();

        let mut stream = FuturesUnordered::new();
        stream.push(self.storage.insert_block(ctx.clone(), block).compat());
        stream.push(
            self.storage
                .update_latest_proof(ctx.clone(), latest_proof.clone())
                .compat(),
        );
        if !signed_txs.is_empty() {
            let tx_positions = build_tx_potsitions(&block_hash, &signed_txs);

            stream.push(
                self.storage
                    .insert_transactions(ctx.clone(), signed_txs)
                    .compat(),
            );
            stream.push(
                self.storage
                    .insert_receipts(ctx.clone(), execution_result.receipts)
                    .compat(),
            );
            stream.push(
                self.storage
                    .insert_transaction_positions(ctx.clone(), tx_positions)
                    .compat(),
            );
        }

        await!(stream.try_collect())?;

        // flush transaction pool
        await!(self
            .tx_pool
            .flush(ctx.clone(), &proposal.tx_hashes)
            .compat())?;

        // update status
        let updated_status = self.update_status(&cloned_header, &block_hash, &latest_proof)?;
        log::info!("block committed, status = {:?}", updated_status);

        Ok(updated_status)
    }

    pub(crate) fn get_status(&self) -> ConsensusResult<ConsensusStatus> {
        let status = self
            .status
            .read()
            .map_err(|_| ConsensusError::Internal("rwlock error".to_owned()))?
            .clone();

        Ok(status)
    }

    pub(crate) fn update_status(
        &self,
        header: &BlockHeader,
        block_hash: &Hash,
        latest_proof: &Proof,
    ) -> ConsensusResult<ConsensusStatus> {
        let mut status = self
            .status
            .write()
            .map_err(|_| ConsensusError::Internal("rwlock error".to_owned()))?;

        status.height = header.height;
        status.timestamp = header.timestamp;
        status.block_hash = block_hash.clone();
        status.state_root = header.state_root.clone();
        status.proof = latest_proof.clone();
        Ok(status.clone())
    }
}

fn build_tx_potsitions(
    block_hash: &Hash,
    signed_txs: &[SignedTransaction],
) -> HashMap<Hash, TransactionPosition> {
    let mut positions = HashMap::with_capacity(signed_txs.len());

    for (position, tx) in signed_txs.iter().enumerate() {
        let tx_position = TransactionPosition {
            block_hash: block_hash.clone(),
            position:   position as u32,
        };
        positions.insert(tx.hash.clone(), tx_position);
    }

    positions
}

fn build_block(proposal: &Proposal, execution_result: &ExecutionResult) -> Block {
    let header = BlockHeader {
        prevhash:          proposal.prevhash.clone(),
        timestamp:         proposal.timestamp,
        height:            proposal.height,
        state_root:        execution_result.state_root.clone(),
        transactions_root: Merkle::from_hashes(proposal.tx_hashes.clone()).get_root_hash(),
        receipts_root:     Merkle::from_receipts(&execution_result.receipts).get_root_hash(),
        logs_bloom:        execution_result.all_logs_bloom,
        quota_limit:       proposal.quota_limit,
        quota_used:        execution_result
            .receipts
            .iter()
            .fold(0, |acc, r| acc + r.quota_used),
        proposer:          proposal.proposer.clone(),
        proof:             proposal.proof.clone(),
    };
    let hash = header.hash();
    Block {
        header,
        hash,
        tx_hashes: proposal.tx_hashes.clone(),
    }
}

fn check_timestamp(current_timestamp: u64, parent_timestamp: u64, interval: u64) -> bool {
    if current_timestamp < parent_timestamp {
        return false;
    }
    current_timestamp < (time_now() + interval)
}

fn time_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
