use std::collections::HashMap;
use std::sync::Arc;

use futures::future::{err, ok, Future};
use futures_locks::RwLock;

use core_crypto::{Crypto, CryptoTransform};
use core_runtime::{FutRuntimeResult, TransactionPool, TransactionPoolError};
use core_storage::{errors::StorageError, storage::Storage};
use core_types::{Address, Hash, SignedTransaction, Transaction, UnverifiedTransaction};

pub struct HashTransactionPool<S> {
    pool_size: usize,
    until_block_limit: u64,
    quota_limit: u64,

    tx_cache: Arc<RwLock<HashMap<Hash, SignedTransaction>>>,
    storage: Arc<S>,
}

impl<S> HashTransactionPool<S>
where
    S: Storage,
{
    pub fn new(
        storage: Arc<S>,
        pool_size: usize,
        until_block_limit: u64,
        quota_limit: u64,
    ) -> Self {
        HashTransactionPool {
            pool_size,
            until_block_limit,
            quota_limit,

            tx_cache: Arc::new(RwLock::new(HashMap::new())),
            storage,
        }
    }
}

impl<S: 'static> TransactionPool for HashTransactionPool<S>
where
    S: Storage,
{
    fn insert<C: Crypto>(
        &self,
        untx: UnverifiedTransaction,
    ) -> FutRuntimeResult<SignedTransaction, TransactionPoolError> {
        let tx_hash = untx.transaction.hash();
        let pool_size = self.pool_size;
        let until_block_limit = self.until_block_limit;
        let quota_limit = self.quota_limit;

        let signature = match C::Signature::from_bytes(&untx.signature) {
            Ok(signatue) => signatue,
            Err(e) => return Box::new(err(TransactionPoolError::Crypto(e))),
        };

        // 1. verify signature
        let sender = match C::verify_with_signature(&tx_hash, &signature) {
            Ok(pubkey) => {
                let hash = Hash::from_raw(&pubkey.as_bytes()[1..]);
                Address::from(&hash.as_ref()[12..])
            }
            Err(e) => return Box::new(err(TransactionPoolError::Crypto(e))),
        };

        // 2. check if the transaction is in histories block.
        match self.storage.get_transaction(&tx_hash).wait() {
            Ok(_) => return Box::new(err(TransactionPoolError::Dup)),
            Err(e) => {
                if !StorageError::is_database_not_found(e.clone()) {
                    return Box::new(err(TransactionPoolError::Internal(e.to_string())));
                }
            }
        };

        let fut = self
            .storage
            .get_latest_block()
            .map_err(|e| TransactionPoolError::Internal(e.to_string()))
            .join(self.tx_cache.write().map_err(map_rwlock_err))
            .and_then(move |(block, mut tx_cache)| {
                // 3. verify params
                if let Err(e) = verify_transaction(
                    block.header.height,
                    &untx.transaction,
                    until_block_limit,
                    quota_limit,
                ) {
                    return err(e);
                }

                // 4. check size
                if tx_cache.len() >= pool_size {
                    return err(TransactionPoolError::ReachLimit);
                }

                // 5. check cache dup
                if tx_cache.contains_key(&tx_hash) {
                    return err(TransactionPoolError::Dup);
                }

                let signed_tx = SignedTransaction {
                    untx,
                    sender,
                    hash: tx_hash.clone(),
                };

                let cloned_signed = signed_tx.clone();
                // 6. insert to cache
                tx_cache.insert(tx_hash.clone(), signed_tx);
                ok(cloned_signed)
                // TODO：7. broadcast the transaction, but event modules are not ready.
            });

        Box::new(fut)
    }

    fn package(
        &self,
        count: u64,
        quota_limit: u64,
    ) -> FutRuntimeResult<Vec<Hash>, TransactionPoolError> {
        let until_block_limit = self.until_block_limit;

        let fut = self
            .storage
            .get_latest_block()
            .map_err(|e| TransactionPoolError::Internal(e.to_string()))
            .join(self.tx_cache.write().map_err(map_rwlock_err))
            .and_then(move |(block, mut tx_cache)| {
                let mut invalid_hashes = vec![];
                let mut valid_hashes = vec![];
                let mut quota_count = 0;

                for (tx_hash, signed_tx) in tx_cache.iter_mut() {
                    let valid_until_block = signed_tx.untx.transaction.valid_until_block;
                    let quota = signed_tx.untx.transaction.quota;

                    if valid_hashes.len() >= count as usize {
                        break;
                    }

                    // The transaction has timed out？
                    if !verify_until_block(
                        valid_until_block,
                        block.header.height,
                        until_block_limit,
                    ) {
                        invalid_hashes.push(tx_hash.clone());
                        continue;
                    }

                    if quota_count + quota > quota_limit {
                        break;
                    }

                    quota_count += quota;
                    valid_hashes.push(tx_hash.clone());
                }

                for h in invalid_hashes {
                    tx_cache.remove(&h);
                }

                ok(valid_hashes)
            });

        Box::new(fut)
    }

    fn flush(&self, tx_hashes: &[Hash]) -> FutRuntimeResult<(), TransactionPoolError> {
        let mut tx_cache = match self.tx_cache.write().wait() {
            Ok(tx_cache) => tx_cache,
            Err(()) => return Box::new(err(map_rwlock_err(()))),
        };

        tx_hashes.iter().for_each(|hash| {
            tx_cache.remove(hash);
        });

        Box::new(ok(()))
    }

    fn get_batch(
        &self,
        tx_hashes: &[Hash],
    ) -> FutRuntimeResult<Vec<Option<SignedTransaction>>, TransactionPoolError> {
        let tx_cache = match self.tx_cache.read().wait() {
            Ok(tx_cache) => tx_cache,
            Err(()) => return Box::new(err(map_rwlock_err(()))),
        };

        let signed_txs = tx_hashes
            .iter()
            .map(|hash| tx_cache.get(hash).cloned())
            .collect();
        Box::new(ok(signed_txs))
    }

    /// TODO: Implement "ensure"
    /// In the POC-1 phase, we only support single-node, so this function is not implemented.
    fn ensure(&self, _tx_hashes: &[Hash]) -> FutRuntimeResult<bool, TransactionPoolError> {
        unimplemented!();
    }
}

fn verify_transaction(
    height: u64,
    tx: &Transaction,
    until_block_limit: u64,
    quota_limit: u64,
) -> Result<(), TransactionPoolError> {
    // verify until block
    if !verify_until_block(tx.valid_until_block, height, until_block_limit) {
        return Err(TransactionPoolError::InvalidUntilBlock);
    }

    // TODO: chain id

    // verify quota
    if tx.quota > quota_limit {
        return Err(TransactionPoolError::QuotaNotEnough);
    }

    Ok(())
}

fn verify_until_block(valid_until_block: u64, current_height: u64, limit_until_block: u64) -> bool {
    !(valid_until_block <= current_height || valid_until_block > current_height + limit_until_block)
}

fn map_rwlock_err(_: ()) -> TransactionPoolError {
    TransactionPoolError::Internal("rwlock error".to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::future::Future;

    use components_database::memory::MemoryDB;
    use core_crypto::{secp256k1::Secp256k1, Crypto, CryptoTransform};
    use core_runtime::{TransactionPool, TransactionPoolError};
    use core_storage::storage::{BlockStorage, Storage};
    use core_types::{Address, Block, Hash, SignedTransaction, Transaction, UnverifiedTransaction};

    use super::HashTransactionPool;

    #[test]
    fn test_insert_transaction() {
        let pool_size = 1000;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(&block).wait().unwrap();

        let tx_pool = HashTransactionPool::new(storage, pool_size, until_block_limit, quota_limit);

        // test normal
        let untx = mock_transaction(100, height + until_block_limit, "test_normal".to_owned());
        let tx_hash = untx.transaction.hash();
        let signed_tx = tx_pool.insert::<Secp256k1>(untx).wait().unwrap();
        assert_eq!(signed_tx.hash, tx_hash);

        // test lt valid_until_block
        let untx = mock_transaction(100, height, "test_lt_quota_limit".to_owned());
        let result = tx_pool.insert::<Secp256k1>(untx).wait();
        assert_eq!(result, Err(TransactionPoolError::InvalidUntilBlock));

        // test gt valid_until_block
        let untx = mock_transaction(
            100,
            height + until_block_limit * 2,
            "test_gt_valid_until_block".to_owned(),
        );
        let result = tx_pool.insert::<Secp256k1>(untx).wait();
        assert_eq!(result, Err(TransactionPoolError::InvalidUntilBlock));

        // test gt quota limit
        let untx = mock_transaction(
            quota_limit + 1,
            height + until_block_limit,
            "test_gt_quota_limit".to_owned(),
        );
        let result = tx_pool.insert::<Secp256k1>(untx).wait();
        assert_eq!(result, Err(TransactionPoolError::QuotaNotEnough));

        // test cache dup
        let untx = mock_transaction(100, height + until_block_limit, "test_dup".to_owned());
        let untx2 = untx.clone();
        tx_pool.insert::<Secp256k1>(untx).wait().unwrap();
        let result = tx_pool.insert::<Secp256k1>(untx2).wait();
        assert_eq!(result, Err(TransactionPoolError::Dup));
    }

    #[test]
    fn test_histories_dup() {
        let pool_size = 1000;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let signed_tx = mock_signed_transaction(
            100,
            height + until_block_limit,
            "test_histories_dup".to_owned(),
        );
        storage
            .insert_transactions(&[signed_tx.clone()])
            .wait()
            .unwrap();
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(&block).wait().unwrap();

        let tx_pool = HashTransactionPool::new(storage, pool_size, until_block_limit, quota_limit);

        let result = tx_pool.insert::<Secp256k1>(signed_tx.untx).wait();
        assert_eq!(result, Err(TransactionPoolError::Dup));
    }

    #[test]
    fn test_pool_size() {
        let pool_size = 1;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(&block).wait().unwrap();

        let tx_pool = HashTransactionPool::new(storage, pool_size, until_block_limit, quota_limit);

        let untx = mock_transaction(100, height + until_block_limit, "test1".to_owned());
        let tx_hash = untx.transaction.hash();
        let signed_tx = tx_pool.insert::<Secp256k1>(untx).wait().unwrap();
        assert_eq!(signed_tx.hash, tx_hash);

        let untx = mock_transaction(100, height + until_block_limit, "test2".to_owned());
        let result = tx_pool.insert::<Secp256k1>(untx).wait();
        assert_eq!(result, Err(TransactionPoolError::ReachLimit));
    }

    #[test]
    fn test_package_transaction_count() {
        let pool_size = 100;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(&block).wait().unwrap();

        let tx_pool = HashTransactionPool::new(storage, pool_size, until_block_limit, quota_limit);

        let mut tx_hashes = vec![];
        for i in 0..10 {
            let untx = mock_transaction(100, height + until_block_limit, format!("test{}", i));
            tx_hashes.push(untx.transaction.hash());
            tx_pool.insert::<Secp256k1>(untx).wait().unwrap();
        }

        let pachage_tx_hashes = tx_pool
            .package(tx_hashes.len() as u64, quota_limit)
            .wait()
            .unwrap();
        assert_eq!(tx_hashes.len(), pachage_tx_hashes.len());
        assert_eq!(
            tx_hashes
                .iter()
                .any(|hash| !pachage_tx_hashes.contains(hash)),
            false
        );
    }

    #[test]
    fn test_package_transaction_quota_limit() {
        let pool_size = 100;
        let until_block_limit = 100;
        let quota_limit = 800;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(&block).wait().unwrap();

        let tx_pool = HashTransactionPool::new(storage, pool_size, until_block_limit, quota_limit);

        let mut tx_hashes = vec![];
        for i in 0..10 {
            let untx = mock_transaction(100, height + until_block_limit, format!("test{}", i));
            tx_hashes.push(untx.transaction.hash());
            tx_pool.insert::<Secp256k1>(untx).wait().unwrap();
        }

        let pachage_tx_hashes = tx_pool
            .package(tx_hashes.len() as u64, quota_limit)
            .wait()
            .unwrap();
        assert_eq!(8, pachage_tx_hashes.len());
    }

    fn mock_transaction(
        quota: u64,
        valid_until_block: u64,
        nonce: String,
    ) -> UnverifiedTransaction {
        let (privkey, _pubkey) = Secp256k1::gen_keypair();
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
        tx.value = vec![];
        tx.chain_id = vec![];
        let tx_hash = tx.hash();

        let signature = Secp256k1::sign(&tx_hash, &privkey).unwrap();
        UnverifiedTransaction {
            transaction: tx,
            signature: signature.as_bytes().to_vec(),
        }
    }

    fn mock_signed_transaction(
        quota: u64,
        valid_until_block: u64,
        nonce: String,
    ) -> SignedTransaction {
        let (privkey, pubkey) = Secp256k1::gen_keypair();
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
        tx.value = vec![];
        tx.chain_id = vec![];
        let tx_hash = tx.hash();

        let signature = Secp256k1::sign(&tx_hash, &privkey).unwrap();
        let untx = UnverifiedTransaction {
            transaction: tx,
            signature: signature.as_bytes().to_vec(),
        };

        SignedTransaction {
            untx: untx.clone(),
            hash: untx.transaction.hash(),
            sender: {
                let hash = Hash::from_raw(&pubkey.as_bytes()[1..]);
                Address::from(&hash.as_ref()[12..])
            },
        }
    }
}
