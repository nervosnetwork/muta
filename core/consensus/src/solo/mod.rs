use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::future::{ok, Future};
use futures_locks::RwLock;

use core_crypto::{Crypto, CryptoTransform};
use core_merkel::Merkel;
use core_runtime::{Executor, TransactionPool};
use core_storage::storage::Storage;
use core_types::{Address, Block, BlockHeader, Hash};

use crate::errors::ConsensusError;

pub struct Status {
    pub height: u64,
    pub block_hash: Hash,
    pub state_root: Hash,
}

pub struct Solo<E, T, S, C>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
{
    executor: Arc<E>,
    tx_pool: Arc<T>,
    storage: Arc<S>,

    // privkey: C::PrivateKey,
    address: Address,
    quota_limit: u64,
    transaction_size: u64,
    status: Arc<RwLock<Status>>,

    _phantom_data: PhantomData<C>,
}

impl<E, T, S, C> Solo<E, T, S, C>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
{
    pub fn new(
        executor: Arc<E>,
        tx_pool: Arc<T>,
        storage: Arc<S>,
        privkey: C::PrivateKey,
        quota_limit: u64,
        transaction_size: u64,
    ) -> Result<Self, ConsensusError> {
        let pubkey = C::get_public_key(&privkey)?;
        let pubkey_hash = Hash::from_raw(pubkey.as_bytes());
        let address = Address::from_hash(&pubkey_hash);

        let current_block = storage.get_latest_block().wait()?;
        let status = Status {
            height: current_block.header.height,
            block_hash: current_block.header.hash(),
            state_root: current_block.header.state_root.clone(),
        };

        Ok(Solo {
            executor,
            tx_pool,
            storage,

            // privkey,
            address,
            quota_limit,
            transaction_size,
            status: Arc::new(RwLock::new(status)),

            _phantom_data: PhantomData::<C>,
        })
    }

    pub fn boom(&self) -> impl Future<Item = (), Error = ConsensusError> + '_ {
        self.tx_pool
            .package(self.transaction_size, self.quota_limit)
            .map_err(ConsensusError::TransactionPool)
            // Proposal block
            .and_then(move |tx_hashes| ok(self.build_proposal_block(tx_hashes)))
            // Let's pretend that we have completed a proposal.
            .and_then(move |mut next_block| {
                log::info!(target: "consensus", "next height = {} transaction len = {}", next_block.header.height, next_block.tx_hashes.len());
                let opt_signed_txs = self
                    .tx_pool
                    .get_batch(&next_block.tx_hashes)
                    .wait()
                    .unwrap();

                let mut signed_txs = Vec::with_capacity(opt_signed_txs.len());
                for opt_signed_tx in opt_signed_txs {
                    match opt_signed_tx {
                        Some(signed_tx) => signed_txs.push(signed_tx),
                        None => panic!("Transaction cannot be empty"),
                    }
                }

                let status = self.status.read().wait().unwrap();

                let execution_result = self
                    .executor
                    .exec(&status.state_root, &next_block.header, &signed_txs)
                    .unwrap();
                next_block.header.state_root = execution_result.state_root;
                next_block.header.logs_bloom = execution_result.all_logs_bloom;
                let receipts_root =
                    Merkel::receipts_root(&execution_result.receipts).unwrap();
                next_block.header.receipts_root = receipts_root;

                ok(next_block.header.clone()).join5(
                    self.status.write().map_err(map_err_rwlock),
                    self.storage
                        .insert_block(&next_block)
                        .map_err(ConsensusError::Storage),
                    self.storage
                        .insert_transactions(&signed_txs)
                        .map_err(ConsensusError::Storage),
                    self.storage
                        .insert_receipts(&execution_result.receipts)
                        .map_err(ConsensusError::Storage),
                )
            })
            .and_then(|(next_block_header, mut status, _, _, _)| {
                status.height = next_block_header.height;
                status.block_hash = next_block_header.hash();
                status.state_root = next_block_header.state_root;
                ok(())
            })
    }

    fn build_proposal_block(&self, tx_hashes: Vec<Hash>) -> Block {
        let status = self.status.read().wait().unwrap();

        let transaction_root = Merkel::hashes_root(&tx_hashes).unwrap();
        let header = BlockHeader {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            prevhash: status.block_hash.clone(),
            height: status.height,
            transactions_root: transaction_root,
            quota_limit: self.quota_limit,
            proposer: self.address.clone(),
            ..Default::default()
        };

        Block { header, tx_hashes }
    }
}

fn map_err_rwlock(_: ()) -> ConsensusError {
    ConsensusError::Internal("rwlock error".to_owned())
}
