use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::future::{ok, Future};
use futures_locks::RwLock;
use tokio::timer::Delay;

use core_crypto::{Crypto, CryptoTransform};
use core_merkle::Merkle;
use core_runtime::{Executor, TransactionPool};
use core_storage::storage::Storage;
use core_types::{Address, Block, BlockHeader, Hash, TransactionPosition};

use crate::errors::ConsensusError;

#[derive(Clone, Debug)]
pub struct Status {
    pub height: u64,
    pub quota_limit: u64,
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
    crypto: Arc<C>,

    // privkey: C::PrivateKey,
    address: Address,
    transaction_size: u64,
    status: Arc<RwLock<Status>>,
}

impl<E: 'static, T: 'static, S: 'static, C: 'static> Solo<E, T, S, C>
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
        crypto: Arc<C>,
        privkey: C::PrivateKey,
        transaction_size: u64,
    ) -> Result<Self, ConsensusError> {
        let pubkey = crypto.get_public_key(&privkey)?;
        let pubkey_hash = Hash::digest(&pubkey.as_bytes()[1..]);
        let address = Address::from_hash(&pubkey_hash);

        let current_block = storage.get_latest_block().wait()?;
        let status = Status {
            height: current_block.header.height,
            quota_limit: current_block.header.quota_limit,
            block_hash: current_block.header.hash(),
            state_root: current_block.header.state_root.clone(),
        };

        Ok(Solo {
            executor,
            tx_pool,
            storage,
            crypto,

            // privkey,
            address,
            transaction_size,
            status: Arc::new(RwLock::new(status)),
        })
    }

    pub fn boom(&self) -> Box<Future<Item = (), Error = ConsensusError> + Send> {
        let status = { self.status.read().wait().unwrap().clone() };

        let quota_limit = status.quota_limit;
        let self_instant1 = self.clone();
        let self_instant2 = self.clone();
        let self_instant3 = self.clone();
        let tx_pool = Arc::clone(&self.tx_pool);
        let tx_pool2 = Arc::clone(&self.tx_pool);
        let storage = Arc::clone(&self.storage);

        let fut = tx_pool
            .package(self.transaction_size, quota_limit)
            .map_err(ConsensusError::TransactionPool)
            // Proposal block
            .and_then(move |tx_hashes| ok(self_instant1.build_proposal_block(tx_hashes)))
            // Let's pretend that we have completed a proposal.
            .and_then(move |mut next_block| {
                let status = status.clone();
                log::info!(target: "consensus", "next height = {:?} transaction len = {:?}", next_block.header.height, next_block.tx_hashes.len());
                if next_block.tx_hashes.is_empty() {
                    next_block.header.state_root = status.state_root.clone();
                    Box::new(ok(next_block))
                } else {
                    self_instant2.exec_block(status.clone(), next_block.clone())
                }
            })
            .and_then(move |block| {
                let tx_hashes = block.tx_hashes.clone();
                ok(block.header.clone()).join4(
                    self_instant3.status.write().map_err(map_err_rwlock),
                    storage.insert_block(block).map_err(ConsensusError::Storage),
                    tx_pool2.flush(&tx_hashes).map_err(ConsensusError::TransactionPool),
                )
            })
            .and_then(|(header, mut status, _, _)| {
                log::info!("block committed, height = {:?} state root = {:?} quota used = {:?}", header.height, header.state_root, header.quota_used);

                status.height = header.height;
                status.quota_limit = header.quota_limit;
                status.block_hash = header.hash();
                status.state_root = header.state_root;
                ok(())
            });
        Box::new(fut)
    }

    fn build_proposal_block(&self, tx_hashes: Vec<Hash>) -> Block {
        let status = self.status.read().wait().unwrap();

        let mut header = BlockHeader {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            prevhash: status.block_hash.clone(),
            height: status.height + 1,
            quota_limit: status.quota_limit,
            proposer: self.address.clone(),
            ..Default::default()
        };

        if !tx_hashes.is_empty() {
            let transactions_root = Merkle::hashes_root(&tx_hashes).unwrap();
            header.transactions_root = transactions_root;
        }

        Block { header, tx_hashes }
    }

    fn exec_block(
        &self,
        status: Status,
        mut block: Block,
    ) -> Box<Future<Item = Block, Error = ConsensusError> + Send> {
        let storage = Arc::clone(&self.storage);
        let executor = Arc::clone(&self.executor);

        let fut = self
            .tx_pool
            .get_batch(&block.tx_hashes)
            .map_err(ConsensusError::TransactionPool)
            .and_then(move |opt_signed_txs| {
                let mut signed_txs = Vec::with_capacity(opt_signed_txs.len());
                for opt_signed_tx in opt_signed_txs {
                    match opt_signed_tx {
                        Some(signed_tx) => signed_txs.push(signed_tx),
                        None => panic!("Transaction cannot be empty"),
                    }
                }

                let execution_result =
                    match executor.exec(&status.state_root, &block.header, &signed_txs) {
                        Ok(execution_result) => execution_result,
                        Err(e) => panic!("exec block: {:?}", e),
                    };
                let receipts_root = Merkle::receipts_root(&execution_result.receipts)
                    .expect("receipts hash cannot be empty");
                let quota_used = execution_result
                    .receipts
                    .iter()
                    .fold(0, |acc, r| acc + r.quota_used);

                block.header.state_root = execution_result.state_root;
                block.header.logs_bloom = execution_result.all_logs_bloom;
                block.header.receipts_root = receipts_root;
                block.header.quota_used = quota_used;

                let mut positions = HashMap::with_capacity(signed_txs.len());
                let block_hash = block.header.hash();

                for (position, tx) in signed_txs.iter().enumerate() {
                    let tx_position = TransactionPosition {
                        block_hash: block_hash.clone(),
                        position: position as u32,
                    };
                    positions.insert(tx.hash.clone(), tx_position);
                }

                ok(block)
                    .join4(
                        storage.insert_transactions(signed_txs),
                        storage.insert_transaction_positions(positions),
                        storage.insert_receipts(execution_result.receipts),
                    )
                    .map_err(ConsensusError::Storage)
            })
            .and_then(|(block, _, _, _)| ok(block));

        Box::new(fut)
    }
}

impl<E: 'static, T, S: 'static, C> Solo<E, T, S, C>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
{
    fn clone(&self) -> Self {
        Solo {
            executor: Arc::clone(&self.executor),
            tx_pool: Arc::clone(&self.tx_pool),
            storage: Arc::clone(&self.storage),
            crypto: Arc::clone(&self.crypto),

            address: self.address.clone(),
            transaction_size: self.transaction_size,
            status: Arc::clone(&self.status),
        }
    }
}

fn map_err_rwlock(_: ()) -> ConsensusError {
    ConsensusError::Internal("rwlock error".to_owned())
}

fn map_err_print(e: Box<Error>) {
    log::error!(target: "solo consensus", "{}", e);
}

pub fn solo_interval<
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
>(
    solo: Arc<Solo<E, T, S, C>>,
    start_time: Instant,
    interval: Duration,
) {
    let solo1 = Arc::clone(&solo);
    let solo2 = Arc::clone(&solo);

    let now = Instant::now();
    let next = if now - start_time > interval {
        now
    } else {
        now + (interval - (now - start_time))
    };

    let dealy = Delay::new(next)
        .map_err(|e| ConsensusError::Internal(e.to_string()))
        .and_then(move |_| solo1.boom())
        .map(move |_| solo_interval(solo2, next, interval));
    tokio::spawn(dealy.map_err(|e| map_err_print(Box::new(e))));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use futures::future::Future;

    use components_database::memory::MemoryDB;
    use components_executor::{
        evm::{EVMBlockDataProvider, EVMExecutor},
        TrieDB,
    };
    use components_transaction_pool::HashTransactionPool;
    use core_crypto::{
        secp256k1::{PrivateKey, Secp256k1},
        Crypto, CryptoTransform,
    };
    use core_runtime::TransactionPool;
    use core_storage::storage::{BlockStorage, Storage};
    use core_types::{
        Address, Block, Genesis, Hash, StateAlloc, Transaction, UnverifiedTransaction,
    };

    use super::Solo;

    #[test]
    fn test_boom() {
        let secp = Arc::new(Secp256k1::new());
        let (privkey, pubkey) = secp.gen_keypair();
        let pubkey_hash = Hash::digest(&pubkey.as_bytes()[1..]);
        let node_address = Address::from_hash(&pubkey_hash);

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(Arc::clone(&db)));
        let tx_pool = Arc::new(HashTransactionPool::new(
            Arc::clone(&storage),
            Arc::clone(&secp),
            100 * 100,
            100,
            1000 * 1024,
        ));
        let trie_db = TrieDB::new(Arc::clone(&db));
        let block_provider = EVMBlockDataProvider::new(Arc::clone(&storage));
        let mut block = Block::default();
        block.header.quota_limit = 1000 * 1024 * 1024 * 1024;
        block.header.height = 1;
        block.header.prevhash = Hash::default();

        let genesis = build_genesis(&node_address, &block);
        let (executor, state_root) =
            EVMExecutor::from_genesis(&genesis, trie_db, Box::new(block_provider)).unwrap();
        block.header.state_root = state_root.clone();
        let height = block.header.height;
        let transactions_root = block.header.transactions_root.clone();
        let receipts_root = block.header.receipts_root.clone();

        storage.insert_block(block).wait().unwrap();
        let executor = Arc::new(executor);
        let solo: Solo<_, _, _, Secp256k1> = Solo::new(
            Arc::clone(&executor),
            Arc::clone(&tx_pool),
            Arc::clone(&storage),
            Arc::clone(&secp),
            privkey.clone(),
            100,
        )
        .unwrap();

        let tx_count = 10;
        for i in 0..tx_count {
            let tx = mock_untx_transaction(
                1000 * 1024,
                hex::decode("ffffff").unwrap(),
                100,
                format!("tx{}", i),
                &privkey,
            );
            tx_pool.insert(tx).wait().unwrap();
        }

        // exec block
        solo.boom().wait().unwrap();

        let block2 = storage.get_latest_block().wait().unwrap();
        assert_eq!(block2.tx_hashes.len(), tx_count);
        assert_eq!(block2.header.height, height + 1);
        assert_ne!(state_root, block2.header.state_root);
        assert_ne!(transactions_root, block2.header.transactions_root);
        assert_ne!(receipts_root, block2.header.receipts_root);
    }

    fn build_genesis(address: &Address, block: &Block) -> Genesis {
        Genesis {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            prevhash: hex::encode(block.header.prevhash.clone().as_bytes()),
            state_alloc: vec![StateAlloc {
                address: hex::encode(address.as_bytes()),
                balance: hex::encode(b"fffffffffffffffff"),
                ..Default::default()
            }],
        }
    }

    fn mock_untx_transaction(
        quota: u64,
        value: Vec<u8>,
        valid_until_block: u64,
        nonce: String,
        signer: &PrivateKey,
    ) -> UnverifiedTransaction {
        let secp = Secp256k1::new();
        let mut tx = Transaction::default();
        tx.to = Address::from_bytes(
            hex::decode("ffffffffffffffffffffffffffffffffffffffff")
                .unwrap()
                .as_ref(),
        )
        .unwrap();
        tx.nonce = nonce;
        tx.quota = quota;
        tx.valid_until_block = valid_until_block;
        tx.data = vec![];
        tx.value = value;
        tx.chain_id = vec![];
        let tx_hash = tx.hash();

        let signature = secp.sign(&tx_hash, signer).unwrap();
        UnverifiedTransaction {
            transaction: tx,
            signature: signature.as_bytes().to_vec(),
        }
    }
}
