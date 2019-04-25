#![feature(async_await, await_macro, futures_api)]

use std::collections::HashMap;
use std::string::ToString;
use std::sync::Arc;

use futures::future::{err, ok, Future};
use futures03::{
    compat::Future01CompatExt,
    prelude::{FutureExt, TryFutureExt},
};
use futures_locks::RwLock;

use core_context::Context;
use core_crypto::{Crypto, CryptoTransform};
use core_runtime::{FutRuntimeResult, TransactionPool, TransactionPoolError};
use core_storage::{Storage, StorageError};
use core_types::{Address, Hash, SignedTransaction, Transaction, UnverifiedTransaction};

pub struct HashTransactionPool<S, C> {
    pool_size:         usize,
    until_block_limit: u64,
    quota_limit:       u64,

    tx_cache: Arc<RwLock<HashMap<Hash, SignedTransaction>>>,
    storage:  Arc<S>,
    crypto:   Arc<C>,
}

impl<S, C> HashTransactionPool<S, C>
where
    S: Storage,
    C: Crypto,
{
    pub fn new(
        storage: Arc<S>,
        crypto: Arc<C>,
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
            crypto,
        }
    }
}

impl<S: 'static, C: 'static> TransactionPool for HashTransactionPool<S, C>
where
    S: Storage,
    C: Crypto,
{
    fn insert(
        &self,
        ctx: Context,
        tx_hash: Hash,
        untx: UnverifiedTransaction,
    ) -> FutRuntimeResult<SignedTransaction, TransactionPoolError> {
        let storage = Arc::clone(&self.storage);
        let crypto = Arc::clone(&self.crypto);
        let tx_cache = Arc::clone(&self.tx_cache);

        let pool_size = self.pool_size;
        let until_block_limit = self.until_block_limit;
        let quota_limit = self.quota_limit;

        let fut = async move {
            // 1. verify signature
            let signature =
                C::Signature::from_bytes(&untx.signature).map_err(TransactionPoolError::Crypto)?;

            // recover sender
            let pubkey = crypto
                .verify_with_signature(&tx_hash, &signature)
                .map_err(TransactionPoolError::Crypto)?;

            let sender = {
                let hash = Hash::digest(&pubkey.as_bytes()[1..]);
                Address::from_hash(&hash)
            };

            // 2. check if the transaction is in histories block.
            match await!(storage.get_transaction(ctx.clone(), &tx_hash).compat()) {
                Ok(_) => Err(TransactionPoolError::Dup)?,
                Err(StorageError::None(_)) => {}
                Err(e) => Err(internal_error(e))?,
            }

            let mut tx_cache_w = await!(tx_cache.write().compat()).map_err(map_rwlock_err)?;

            // 3. verify params
            let latest_block =
                await!(storage.get_latest_block(ctx).compat()).map_err(internal_error)?;

            verify_transaction(
                latest_block.header.height,
                &untx.transaction,
                until_block_limit,
                quota_limit,
            )?;

            // 4. check size
            if tx_cache_w.len() >= pool_size {
                Err(TransactionPoolError::ReachLimit)?
            }

            // 5. check cache dup
            if tx_cache_w.contains_key(&tx_hash) {
                Err(TransactionPoolError::Dup)?
            }

            // 6. do insert
            let signed_tx = SignedTransaction {
                untx,
                sender,
                hash: tx_hash.clone(),
            };

            tx_cache_w.insert(tx_hash, signed_tx.clone());
            Ok(signed_tx)
        };

        Box::new(fut.boxed().compat())
    }

    fn package(
        &self,
        ctx: Context,
        count: u64,
        quota_limit: u64,
    ) -> FutRuntimeResult<Vec<Hash>, TransactionPoolError> {
        let storage = Arc::clone(&self.storage);
        let tx_cache = Arc::clone(&self.tx_cache);
        let until_block_limit = self.until_block_limit;

        let fut = async move {
            let latest_block =
                await!(storage.get_latest_block(ctx).compat()).map_err(internal_error)?;

            let mut invalid_hashes = vec![];
            let mut valid_hashes = vec![];
            let mut quota_count: u64 = 0;

            let mut tx_cache_w = await!(tx_cache.write().compat()).map_err(map_rwlock_err)?;

            for (tx_hash, signed_tx) in tx_cache_w.iter_mut() {
                let valid_until_block = signed_tx.untx.transaction.valid_until_block;
                let quota = signed_tx.untx.transaction.quota;

                if valid_hashes.len() >= count as usize {
                    break;
                }

                // The transaction has timed out？
                if !verify_until_block(
                    valid_until_block,
                    latest_block.header.height,
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
                tx_cache_w.remove(&h);
            }

            Ok(valid_hashes)
        };

        Box::new(fut.boxed().compat())
    }

    fn flush(&self, _: Context, tx_hashes: &[Hash]) -> FutRuntimeResult<(), TransactionPoolError> {
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
        _: Context,
        tx_hashes: &[Hash],
    ) -> FutRuntimeResult<Vec<SignedTransaction>, TransactionPoolError> {
        let tx_cache = match self.tx_cache.read().wait() {
            Ok(tx_cache) => tx_cache,
            Err(()) => return Box::new(err(map_rwlock_err(()))),
        };

        let signed_txs = tx_hashes
            .iter()
            .map(|hash| tx_cache.get(hash).cloned())
            .filter_map(|tx| tx)
            .collect();
        Box::new(ok(signed_txs))
    }

    /// TODO: Implement "ensure"
    /// In the POC-1 phase, we only support single-node, so this function is not
    /// implemented.
    fn ensure(
        &self,
        _: Context,
        _tx_hashes: &[Hash],
    ) -> FutRuntimeResult<(), TransactionPoolError> {
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

fn internal_error(e: impl ToString) -> TransactionPoolError {
    TransactionPoolError::Internal(e.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::future::Future;

    use components_database::memory::MemoryDB;
    use core_context::Context;
    use core_crypto::{secp256k1::Secp256k1, Crypto, CryptoTransform};
    use core_runtime::{TransactionPool, TransactionPoolError};
    use core_storage::{BlockStorage, Storage};
    use core_types::{Address, Block, Hash, SignedTransaction, Transaction, UnverifiedTransaction};

    use super::HashTransactionPool;

    #[test]
    fn test_insert_transaction() {
        let ctx = Context::new();
        let secp = Arc::new(Secp256k1::new());
        let pool_size = 1000;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(ctx.clone(), block).wait().unwrap();

        let tx_pool =
            HashTransactionPool::new(storage, secp, pool_size, until_block_limit, quota_limit);

        // test normal
        let untx = mock_transaction(100, height + until_block_limit, "test_normal".to_owned());
        let tx_hash = untx.transaction.hash();
        let signed_tx = tx_pool
            .insert(ctx.clone(), tx_hash.clone(), untx)
            .wait()
            .unwrap();
        assert_eq!(signed_tx.hash, tx_hash);

        // test lt valid_until_block
        let untx = mock_transaction(100, height, "test_lt_quota_limit".to_owned());
        let tx_hash = untx.transaction.hash();
        let result = tx_pool.insert(ctx.clone(), tx_hash, untx).wait();
        assert_eq!(result, Err(TransactionPoolError::InvalidUntilBlock));

        // test gt valid_until_block
        let untx = mock_transaction(
            100,
            height + until_block_limit * 2,
            "test_gt_valid_until_block".to_owned(),
        );
        let tx_hash = untx.transaction.hash();
        let result = tx_pool.insert(ctx.clone(), tx_hash, untx).wait();
        assert_eq!(result, Err(TransactionPoolError::InvalidUntilBlock));

        // test gt quota limit
        let untx = mock_transaction(
            quota_limit + 1,
            height + until_block_limit,
            "test_gt_quota_limit".to_owned(),
        );
        let tx_hash = untx.transaction.hash();
        let result = tx_pool.insert(ctx.clone(), tx_hash, untx).wait();
        assert_eq!(result, Err(TransactionPoolError::QuotaNotEnough));

        // test cache dup
        let untx = mock_transaction(100, height + until_block_limit, "test_dup".to_owned());
        let untx2 = untx.clone();
        let tx_hash = untx.transaction.hash();
        let tx_hash2 = untx2.transaction.hash();
        tx_pool.insert(ctx.clone(), tx_hash, untx).wait().unwrap();
        let result = tx_pool.insert(ctx.clone(), tx_hash2, untx2).wait();
        assert_eq!(result, Err(TransactionPoolError::Dup));
    }

    #[test]
    fn test_histories_dup() {
        let ctx = Context::new();
        let secp = Arc::new(Secp256k1::new());
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

        let mut block = Block::default();
        block.header.height = height;

        storage
            .insert_transactions(ctx.clone(), vec![signed_tx.clone()])
            .wait()
            .unwrap();

        storage.insert_block(ctx.clone(), block).wait().unwrap();

        let tx_pool =
            HashTransactionPool::new(storage, secp, pool_size, until_block_limit, quota_limit);

        let tx_hash = signed_tx.untx.transaction.hash();
        let result = tx_pool.insert(ctx.clone(), tx_hash, signed_tx.untx).wait();
        assert_eq!(result, Err(TransactionPoolError::Dup));
    }

    #[test]
    fn test_pool_size() {
        let ctx = Context::new();
        let secp = Arc::new(Secp256k1::new());
        let pool_size = 1;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(ctx.clone(), block).wait().unwrap();

        let tx_pool =
            HashTransactionPool::new(storage, secp, pool_size, until_block_limit, quota_limit);

        let untx = mock_transaction(100, height + until_block_limit, "test1".to_owned());
        let tx_hash = untx.transaction.hash();
        let signed_tx = tx_pool
            .insert(ctx.clone(), tx_hash.clone(), untx)
            .wait()
            .unwrap();
        assert_eq!(signed_tx.hash, tx_hash);

        let untx = mock_transaction(100, height + until_block_limit, "test2".to_owned());
        let tx_hash = untx.transaction.hash();
        let result = tx_pool.insert(ctx.clone(), tx_hash, untx).wait();
        assert_eq!(result, Err(TransactionPoolError::ReachLimit));
    }

    #[test]
    fn test_package_transaction_count() {
        let ctx = Context::new();
        let secp = Arc::new(Secp256k1::new());
        let pool_size = 100;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(ctx.clone(), block).wait().unwrap();

        let tx_pool =
            HashTransactionPool::new(storage, secp, pool_size, until_block_limit, quota_limit);

        let mut tx_hashes = vec![];
        for i in 0..10 {
            let untx = mock_transaction(100, height + until_block_limit, format!("test{}", i));
            let tx_hash = untx.transaction.hash();
            tx_hashes.push(tx_hash.clone());
            tx_pool.insert(ctx.clone(), tx_hash, untx).wait().unwrap();
        }

        let pachage_tx_hashes = tx_pool
            .package(ctx, tx_hashes.len() as u64, quota_limit)
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
        let ctx = Context::new();
        let secp = Arc::new(Secp256k1::new());
        let pool_size = 100;
        let until_block_limit = 100;
        let quota_limit = 800;
        let height = 100;

        let db = Arc::new(MemoryDB::new());
        let storage = Arc::new(BlockStorage::new(db));
        let mut block = Block::default();
        block.header.height = height;
        storage.insert_block(ctx.clone(), block).wait().unwrap();

        let tx_pool =
            HashTransactionPool::new(storage, secp, pool_size, until_block_limit, quota_limit);

        let mut tx_hashes = vec![];
        for i in 0..10 {
            let untx = mock_transaction(100, height + until_block_limit, format!("test{}", i));
            let tx_hash = untx.transaction.hash();
            tx_hashes.push(tx_hash.clone());
            tx_pool.insert(ctx.clone(), tx_hash, untx).wait().unwrap();
        }

        let pachage_tx_hashes = tx_pool
            .package(ctx, tx_hashes.len() as u64, quota_limit)
            .wait()
            .unwrap();
        assert_eq!(8, pachage_tx_hashes.len());
    }

    fn mock_transaction(
        quota: u64,
        valid_until_block: u64,
        nonce: String,
    ) -> UnverifiedTransaction {
        let secp = Secp256k1::new();
        let (privkey, _pubkey) = secp.gen_keypair();
        let mut tx = Transaction::default();
        tx.to = Some(
            Address::from_bytes(
                hex::decode("ffffffffffffffffffffffffffffffffffffffff")
                    .unwrap()
                    .as_ref(),
            )
            .unwrap(),
        );
        tx.nonce = nonce;
        tx.quota = quota;
        tx.valid_until_block = valid_until_block;
        tx.data = vec![];
        tx.value = vec![];
        tx.chain_id = vec![];
        let tx_hash = tx.hash();

        let signature = secp.sign(&tx_hash, &privkey).unwrap();
        UnverifiedTransaction {
            transaction: tx,
            signature:   signature.as_bytes().to_vec(),
        }
    }

    fn mock_signed_transaction(
        quota: u64,
        valid_until_block: u64,
        nonce: String,
    ) -> SignedTransaction {
        let secp = Secp256k1::new();
        let (privkey, pubkey) = secp.gen_keypair();
        let mut tx = Transaction::default();
        tx.to = Some(
            Address::from_bytes(
                hex::decode("ffffffffffffffffffffffffffffffffffffffff")
                    .unwrap()
                    .as_ref(),
            )
            .unwrap(),
        );
        tx.nonce = nonce;
        tx.quota = quota;
        tx.valid_until_block = valid_until_block;
        tx.data = vec![];
        tx.value = vec![];
        tx.chain_id = vec![];
        let tx_hash = tx.hash();

        let signature = secp.sign(&tx_hash, &privkey).unwrap();
        let untx = UnverifiedTransaction {
            transaction: tx,
            signature:   signature.as_bytes().to_vec(),
        };

        SignedTransaction {
            untx:   untx.clone(),
            hash:   untx.transaction.hash(),
            sender: {
                let hash = Hash::digest(&pubkey.as_bytes()[1..]);
                Address::from_hash(&hash)
            },
        }
    }
}
