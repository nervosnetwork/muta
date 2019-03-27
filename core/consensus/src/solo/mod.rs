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

    // privkey: C::PrivateKey,
    address: Address,
    transaction_size: u64,
    status: Arc<RwLock<Status>>,

    _phantom_data: PhantomData<C>,
}

impl<E: 'static, T, S: 'static, C> Solo<E, T, S, C>
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
        transaction_size: u64,
    ) -> Result<Self, ConsensusError> {
        let pubkey = C::get_public_key(&privkey)?;
        let pubkey_hash = Hash::from_raw(pubkey.as_bytes());
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

            // privkey,
            address,
            transaction_size,
            status: Arc::new(RwLock::new(status)),

            _phantom_data: PhantomData::<C>,
        })
    }

    pub fn boom(&self) -> impl Future<Item = (), Error = ConsensusError> + '_ {
        let quota_limit = { self.status.read().wait().unwrap().quota_limit };

        self.tx_pool
            .package(self.transaction_size, quota_limit)
            .map_err(ConsensusError::TransactionPool)
            // Proposal block
            .and_then(move |tx_hashes| ok(self.build_proposal_block(tx_hashes)))
            // Let's pretend that we have completed a proposal.
            .and_then(move |mut next_block| {
                log::info!(target: "consensus", "next height = {} transaction len = {}", next_block.header.height, next_block.tx_hashes.len());
                let status = self.status.read().wait().unwrap();

                if next_block.tx_hashes.is_empty() {
                    next_block.header.state_root = status.state_root.clone();
                    Box::new(ok(next_block))
                } else {
                    self.exec_block(status.clone(), next_block.clone())
                }
            })
            .and_then(move |block| {
                ok(block.header.clone()).join3(
                    self.status.write().map_err(map_err_rwlock),
                    self.storage.insert_block(&block).map_err(ConsensusError::Storage),
                )
            })
            .and_then(|(header, mut status, _)| {
                status.height = header.height;
                status.quota_limit = header.quota_limit;
                status.block_hash = header.hash();
                status.state_root = header.state_root;
                ok(())
            })
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
            let transactions_root = Merkel::hashes_root(&tx_hashes).unwrap();
            header.transactions_root = transactions_root;
        }

        Block { header, tx_hashes }
    }

    fn exec_block(
        &self,
        status: Status,
        mut block: Block,
    ) -> Box<Future<Item = Block, Error = ConsensusError>> {
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
                let receipts_root = Merkel::receipts_root(&execution_result.receipts)
                    .expect("receipts hash cannot be empty");
                let quota_used = execution_result
                    .receipts
                    .iter()
                    .fold(0, |acc, r| acc + r.quota_used);

                block.header.state_root = execution_result.state_root;
                block.header.logs_bloom = execution_result.all_logs_bloom;
                block.header.receipts_root = receipts_root;
                block.header.quota_used = quota_used;

                ok(block.clone())
                    .join3(
                        storage.insert_transactions(&signed_txs),
                        storage.insert_receipts(&execution_result.receipts),
                    )
                    .map_err(ConsensusError::Storage)
            })
            .and_then(|(block, _, _)| ok(block));

        Box::new(fut)
    }
}

fn map_err_rwlock(_: ()) -> ConsensusError {
    ConsensusError::Internal("rwlock error".to_owned())
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
        let (privkey, pubkey) = Secp256k1::gen_keypair();
        let pubkey_hash = Hash::from_raw(&pubkey.as_bytes()[1..]);
        let node_address = Address::from_hash(&pubkey_hash);

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(Arc::clone(&db)));
        let tx_pool = Arc::new(HashTransactionPool::new(
            Arc::clone(&storage),
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
        storage.insert_block(&block).wait().unwrap();
        let executor = Arc::new(executor);
        let solo: Solo<_, _, _, Secp256k1> = Solo::new(
            Arc::clone(&executor),
            Arc::clone(&tx_pool),
            Arc::clone(&storage),
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
            tx_pool.insert::<Secp256k1>(tx).wait().unwrap();
        }

        // exec block
        solo.boom().wait().unwrap();

        let block2 = storage.get_latest_block().wait().unwrap();
        assert_eq!(block2.tx_hashes.len(), tx_count);
        assert_eq!(block2.header.height, block.header.height + 1);
        assert_ne!(block.header.state_root, block2.header.state_root);
        assert_ne!(
            block.header.transactions_root,
            block2.header.transactions_root
        );
        assert_ne!(block.header.receipts_root, block2.header.receipts_root);
    }

    fn build_genesis(address: &Address, block: &Block) -> Genesis {
        Genesis {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            prevhash: hex::encode(block.header.prevhash.clone()),
            state_alloc: vec![StateAlloc {
                address: hex::encode(address),
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
        let mut tx = Transaction::default();
        tx.to = Address::from(
            hex::decode("ffffffffffffffffffffffffffffffffffffffff")
                .unwrap()
                .as_ref(),
        );
        tx.nonce = nonce;
        tx.quota = quota;
        tx.valid_until_block = valid_until_block;
        tx.data = vec![];
        tx.value = value;
        tx.chain_id = vec![];
        let tx_hash = tx.hash();

        let signature = Secp256k1::sign(&tx_hash, signer).unwrap();
        UnverifiedTransaction {
            transaction: tx,
            signature: signature.as_bytes().to_vec(),
        }
    }
}
