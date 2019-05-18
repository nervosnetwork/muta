#![feature(async_await, await_macro, integer_atomics, futures_api, test)]

mod cache;

use std::string::ToString;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures03::{
    compat::Future01CompatExt,
    prelude::{FutureExt, TryFutureExt},
};

use core_context::{Context, ORIGIN};
use core_crypto::Crypto;
use core_runtime::{FutRuntimeResult, TransactionOrigin, TransactionPool, TransactionPoolError};
use core_storage::{Storage, StorageError};
use core_types::{Hash, SignedTransaction, Transaction, UnverifiedTransaction};

use crate::cache::TxCache;

pub trait Broadcaster: Send + Sync + Clone {
    fn broadcast_batch(&mut self, txs: Vec<SignedTransaction>);

    fn pull_txs(
        &mut self,
        ctx: Context,
        hashes: Vec<Hash>,
    ) -> FutRuntimeResult<Vec<SignedTransaction>, TransactionPoolError>;
}

pub struct HashTransactionPool<S, C, B> {
    pool_size:         usize,
    until_block_limit: u64,
    quota_limit:       u64,

    tx_cache:       Arc<TxCache>,
    callback_cache: Arc<TxCache>,
    storage:        Arc<S>,
    crypto:         Arc<C>,
    broadcaster:    B,

    latest_height: Arc<AtomicU64>,
}

impl<S, C, B> HashTransactionPool<S, C, B>
where
    S: Storage,
    C: Crypto,
    B: Broadcaster,
{
    pub fn new(
        storage: Arc<S>,
        crypto: Arc<C>,
        broadcaster: B,
        pool_size: usize,
        until_block_limit: u64,
        quota_limit: u64,
        latest_height: u64,
    ) -> Self {
        HashTransactionPool {
            pool_size,
            until_block_limit,
            quota_limit,

            tx_cache: Arc::new(TxCache::new(pool_size)),
            callback_cache: Arc::new(TxCache::new(pool_size)),
            storage,
            crypto,
            broadcaster,

            latest_height: Arc::new(AtomicU64::new(latest_height)),
        }
    }
}

impl<S, C, B> TransactionPool for HashTransactionPool<S, C, B>
where
    S: Storage + 'static,
    C: Crypto + 'static,
    B: Broadcaster + 'static,
{
    fn insert(
        &self,
        ctx: Context,
        untx: UnverifiedTransaction,
    ) -> FutRuntimeResult<SignedTransaction, TransactionPoolError> {
        let storage = Arc::clone(&self.storage);
        let crypto = Arc::clone(&self.crypto);
        let tx_cache = Arc::clone(&self.tx_cache);
        let mut broadcaster = self.broadcaster.clone();

        let pool_size = self.pool_size;
        let until_block_limit = self.until_block_limit;
        let quota_limit = self.quota_limit;
        let latest_height = self.latest_height.load(Ordering::Acquire);

        let fut = async move {
            // 1. check size
            if tx_cache.len() >= pool_size {
                Err(TransactionPoolError::ReachLimit)?
            }

            let cita_untx: common_cita::UnverifiedTransaction = untx.clone().into();
            let tx_hash = cita_untx
                .clone()
                .transaction
                .ok_or_else(|| TransactionPoolError::TransactionNotFound)?
                .hash();

            // 2. check dup
            if tx_cache.contains_key(&tx_hash) {
                Err(TransactionPoolError::Dup)?
            }

            // recover sender
            let pubkey = cita_untx.verify(Arc::<C>::clone(&crypto))?;
            let sender = crypto.pubkey_to_address(&pubkey);

            // 4. check if the transaction is in histories block.
            match await!(storage.get_transaction(ctx.clone(), &tx_hash).compat()) {
                Ok(_) => Err(TransactionPoolError::Dup)?,
                Err(StorageError::None(_)) => {}
                Err(e) => Err(internal_error(e))?,
            }

            // 5. verify params
            verify_transaction(
                latest_height,
                &untx.transaction,
                until_block_limit,
                quota_limit,
            )?;

            // 6. do insert
            let signed_tx = SignedTransaction {
                untx,
                sender,
                hash: tx_hash.clone(),
            };

            tx_cache.insert(signed_tx.clone());

            // 6. broadcast tx
            if let Some(TransactionOrigin::Jsonrpc) = ctx.get::<TransactionOrigin>(ORIGIN) {
                broadcaster.broadcast_batch(vec![signed_tx.clone()]);
            }

            Ok(signed_tx)
        };

        Box::new(fut.boxed().compat())
    }

    fn package(
        &self,
        _: Context,
        count: u64,
        quota_limit: u64,
    ) -> FutRuntimeResult<Vec<Hash>, TransactionPoolError> {
        let tx_cache = Arc::clone(&self.tx_cache);
        let until_block_limit = self.until_block_limit;
        let latest_height = self.latest_height.load(Ordering::Acquire);

        let fut = async move {
            let mut invalid_hashes = vec![];
            let mut valid_hashes = vec![];
            let mut quota_count: u64 = 0;

            let txs = tx_cache.get_count(count as usize);

            for signed_tx in txs {
                let tx_hash = signed_tx.hash.clone();
                let valid_until_block = signed_tx.untx.transaction.valid_until_block;
                let quota = signed_tx.untx.transaction.quota;

                // The transaction has timed out？
                if !verify_until_block(valid_until_block, latest_height, until_block_limit) {
                    invalid_hashes.push(tx_hash.clone());
                    continue;
                }

                if quota_count + quota > quota_limit {
                    break;
                }

                quota_count += quota;
                valid_hashes.push(tx_hash.clone());
            }

            tx_cache.deletes(&invalid_hashes);

            Ok(valid_hashes)
        };

        Box::new(fut.boxed().compat())
    }

    fn flush(&self, _: Context, tx_hashes: &[Hash]) -> FutRuntimeResult<(), TransactionPoolError> {
        let tx_cache = Arc::clone(&self.tx_cache);
        let callback_cache = Arc::clone(&self.callback_cache);
        let latest_height = Arc::clone(&self.latest_height);
        let tx_hashes = tx_hashes.to_owned();

        let fut = async move {
            callback_cache.clear();
            tx_cache.deletes(&tx_hashes);

            latest_height.fetch_add(1, Ordering::Acquire);
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }

    fn get_batch(
        &self,
        _: Context,
        tx_hashes: &[Hash],
    ) -> FutRuntimeResult<Vec<SignedTransaction>, TransactionPoolError> {
        let tx_cache = Arc::clone(&self.tx_cache);
        let callback_cache = Arc::clone(&self.callback_cache);
        let tx_hashes = tx_hashes.to_owned();

        let fut = async move {
            let mut sig_txs = Vec::with_capacity(tx_hashes.len());

            for hash in tx_hashes.iter() {
                if let Some(tx) = tx_cache.get(hash) {
                    sig_txs.push(tx);
                } else if let Some(tx) = callback_cache.get(hash) {
                    sig_txs.push(tx);
                }
            }

            Ok(sig_txs)
        };

        Box::new(fut.boxed().compat())
    }

    fn ensure(
        &self,
        ctx: Context,
        tx_hashes: &[Hash],
    ) -> FutRuntimeResult<(), TransactionPoolError> {
        let tx_cache = Arc::clone(&self.tx_cache);
        let callback_cache = Arc::clone(&self.callback_cache);
        let mut broadcaster = self.broadcaster.clone();
        let tx_hashes = tx_hashes.to_owned();

        let fut = async move {
            let unknown = tx_cache.contains_keys(&tx_hashes);

            if !unknown.is_empty() {
                let sig_txs = await!(broadcaster.pull_txs(ctx, unknown.clone()).compat())?;
                if sig_txs.len() != unknown.len() {
                    return Err(TransactionPoolError::NotExpected);
                }

                for unknown_hash in unknown {
                    if !sig_txs.iter().any(|tx| tx.hash == unknown_hash) {
                        return Err(TransactionPoolError::NotExpected);
                    }
                }
                callback_cache.insert_batch(sig_txs);
            }

            Ok(())
        };

        Box::new(fut.boxed().compat())
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

fn internal_error(e: impl ToString) -> TransactionPoolError {
    TransactionPoolError::Internal(e.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::future::{ok, Future};

    use common_cita::{
        Transaction as CitaTransaction, UnverifiedTransaction as CitaUnverifiedTransaction,
    };
    use components_database::memory::MemoryDB;
    use core_context::{Context, ORIGIN};
    use core_crypto::{secp256k1::Secp256k1, Crypto, CryptoTransform};
    use core_runtime::{
        FutRuntimeResult, TransactionOrigin, TransactionPool, TransactionPoolError,
    };
    use core_storage::{BlockStorage, Storage};
    use core_types::{Address, Block, Hash, SignedTransaction, Transaction, UnverifiedTransaction};

    use super::{Broadcaster, HashTransactionPool};

    #[derive(Clone)]
    struct BroadcastMock;

    impl Broadcaster for BroadcastMock {
        fn broadcast_batch(&mut self, _: Vec<SignedTransaction>) {
            panic!("should not broadcast inserted txs");
        }

        fn pull_txs(
            &mut self,
            _: Context,
            unknown_hashes: Vec<Hash>,
        ) -> FutRuntimeResult<Vec<SignedTransaction>, TransactionPoolError> {
            let mut mock_stxs = Vec::with_capacity(unknown_hashes.len());

            for hash in unknown_hashes.into_iter() {
                let mut stx = SignedTransaction::default();
                stx.hash = hash;
                mock_stxs.push(stx);
            }

            Box::new(ok(mock_stxs))
        }
    }

    #[test]
    fn test_insert_transaction() {
        let ctx = Context::new();
        let pool_size = 1000;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let tx_pool = new_test_pool(
            ctx.clone(),
            None,
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        // test normal
        let untx = mock_transaction(100, height + until_block_limit, "test_normal".to_owned());
        let signed_tx = tx_pool.insert(ctx.clone(), untx.clone()).wait().unwrap();
        assert_eq!(
            signed_tx.hash,
            Into::<CitaUnverifiedTransaction>::into(untx)
                .transaction
                .unwrap()
                .hash()
        );

        // test lt valid_until_block
        let untx = mock_transaction(100, height, "test_lt_quota_limit".to_owned());
        let result = tx_pool.insert(ctx.clone(), untx).wait();
        assert_eq!(result, Err(TransactionPoolError::InvalidUntilBlock));

        // test gt valid_until_block
        let untx = mock_transaction(
            100,
            height + until_block_limit * 2,
            "test_gt_valid_until_block".to_owned(),
        );
        let result = tx_pool.insert(ctx.clone(), untx).wait();
        assert_eq!(result, Err(TransactionPoolError::InvalidUntilBlock));

        // test gt quota limit
        let untx = mock_transaction(
            quota_limit + 1,
            height + until_block_limit,
            "test_gt_quota_limit".to_owned(),
        );
        let result = tx_pool.insert(ctx.clone(), untx).wait();
        assert_eq!(result, Err(TransactionPoolError::QuotaNotEnough));

        // test cache dup
        let untx = mock_transaction(100, height + until_block_limit, "test_dup".to_owned());
        let untx2 = untx.clone();
        tx_pool.insert(ctx.clone(), untx).wait().unwrap();
        let result = tx_pool.insert(ctx.clone(), untx2).wait();
        assert_eq!(result, Err(TransactionPoolError::Dup));
    }

    // only transactions from jsonrpc will trigger broadcasting
    #[test]
    #[should_panic(expected = "should not broadcast inserted txs")]
    fn test_insert_transaction_from_jsonrpc() {
        let ctx = Context::new();
        let pool_size = 1000;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let tx_pool = new_test_pool(
            ctx.clone(),
            None,
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        // test normal
        let untx = mock_transaction(100, height + until_block_limit, "test_normal".to_owned());
        let origin_ctx = ctx.with_value::<TransactionOrigin>(ORIGIN, TransactionOrigin::Jsonrpc);
        tx_pool.insert(origin_ctx, untx).wait().unwrap();
    }

    #[test]
    fn test_histories_dup() {
        let ctx = Context::new();
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

        let tx_pool = new_test_pool(
            ctx.clone(),
            Some(storage),
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        let result = tx_pool.insert(ctx.clone(), signed_tx.untx).wait();
        assert_eq!(result, Err(TransactionPoolError::Dup));
    }

    #[test]
    fn test_pool_size() {
        let ctx = Context::new();
        let pool_size = 1;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let tx_pool = new_test_pool(
            ctx.clone(),
            None,
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        let untx = mock_transaction(100, height + until_block_limit, "test1".to_owned());
        let signed_tx = tx_pool.insert(ctx.clone(), untx.clone()).wait().unwrap();
        assert_eq!(
            signed_tx.hash,
            Into::<CitaUnverifiedTransaction>::into(untx)
                .transaction
                .unwrap()
                .hash()
        );

        let untx = mock_transaction(100, height + until_block_limit, "test2".to_owned());
        let result = tx_pool.insert(ctx.clone(), untx).wait();
        assert_eq!(result, Err(TransactionPoolError::ReachLimit));
    }

    #[test]
    fn test_package_transaction_count() {
        let ctx = Context::new();
        let pool_size = 100;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let tx_pool = new_test_pool(
            ctx.clone(),
            None,
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        let mut tx_hashes = vec![];
        for i in 0..10 {
            let untx = mock_transaction(100, height + until_block_limit, format!("test{}", i));
            let tx_hash = Into::<CitaUnverifiedTransaction>::into(untx.clone())
                .transaction
                .unwrap()
                .hash();
            tx_hashes.push(tx_hash.clone());
            tx_pool.insert(ctx.clone(), untx.clone()).wait().unwrap();
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
    fn test_flush() {
        let ctx = Context::new();
        let pool_size = 1000;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let tx_pool = new_test_pool(
            ctx.clone(),
            None,
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        let mut sig_txs = Vec::with_capacity(10);
        for i in 1..=10 {
            let stx =
                mock_signed_transaction(100, height + until_block_limit, format!("test stx {}", i));

            sig_txs.push(stx);
        }

        let (callback_stxs, pool_stxs) = sig_txs.split_at(5);
        {
            // insert test signed transactions
            let callback_cache = Arc::clone(&tx_pool.callback_cache);
            let pool_cache = Arc::clone(&tx_pool.tx_cache);

            for stx in callback_stxs.iter() {
                callback_cache.insert(stx.clone());
            }
            for stx in pool_stxs.iter() {
                pool_cache.insert(stx.clone());
            }
        }

        let test_hashes = callback_stxs
            .iter()
            .map(|stx| stx.hash.clone())
            .collect::<Vec<Hash>>();
        let stxs = tx_pool
            .get_batch(ctx.clone(), test_hashes.as_slice())
            .wait()
            .unwrap();
        assert_eq!(stxs.len(), test_hashes.len());
        assert_eq!(tx_pool.callback_cache.len(), test_hashes.len());

        let test_hashes = pool_stxs
            .iter()
            .map(|stx| stx.hash.clone())
            .collect::<Vec<Hash>>();
        tx_pool
            .flush(ctx.clone(), test_hashes.as_slice())
            .wait()
            .unwrap();
        assert_eq!(tx_pool.callback_cache.len(), 0);
        assert_eq!(tx_pool.tx_cache.len(), 0);
    }

    #[test]
    fn test_package_transaction_quota_limit() {
        let ctx = Context::new();
        let pool_size = 100;
        let until_block_limit = 100;
        let quota_limit = 800;
        let height = 100;

        let tx_pool = new_test_pool(
            ctx.clone(),
            None,
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        let mut tx_hashes = vec![];
        for i in 0..10 {
            let untx = mock_transaction(100, height + until_block_limit, format!("test{}", i));
            let tx_hash = Into::<CitaUnverifiedTransaction>::into(untx.clone())
                .transaction
                .unwrap()
                .hash();
            tx_hashes.push(tx_hash.clone());
            tx_pool.insert(ctx.clone(), untx).wait().unwrap();
        }

        let pachage_tx_hashes = tx_pool
            .package(ctx, tx_hashes.len() as u64, quota_limit)
            .wait()
            .unwrap();
        assert_eq!(8, pachage_tx_hashes.len());
    }

    #[test]
    fn test_ensure_partial_unknown_hashes() {
        let ctx = Context::new();
        let pool_size = 1000;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let tx_pool = new_test_pool(
            ctx.clone(),
            None,
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        let mut untxs = vec![];
        let mut tx_hashes = vec![];
        for i in 1..=5 {
            let untx = mock_transaction(100, height + until_block_limit, format!("test{}", i));
            let tx_hash = Into::<CitaUnverifiedTransaction>::into(untx.clone())
                .transaction
                .unwrap()
                .hash();
            tx_hashes.push(tx_hash.clone());
            untxs.push(untx);
        }

        tx_pool
            .insert(ctx.clone(), untxs[0].clone())
            .wait()
            .unwrap();
        assert_eq!(tx_pool.tx_cache.len(), 1);

        tx_pool
            .ensure(ctx.clone(), tx_hashes.as_slice())
            .wait()
            .unwrap();
        let callback_cache = tx_pool.callback_cache;

        dbg!(callback_cache.len());
        assert_eq!(callback_cache.len(), 4);
        assert!(!callback_cache.contains_key(&tx_hashes[0]));
        for hash in tx_hashes.iter().take(5).skip(1) {
            assert!(callback_cache.contains_key(hash));
        }
    }

    #[test]
    fn test_ensure_full_known_hashes() {
        let ctx = Context::new();
        let pool_size = 1000;
        let until_block_limit = 100;
        let quota_limit = 10000;
        let height = 100;

        let tx_pool = new_test_pool(
            ctx.clone(),
            None,
            pool_size,
            until_block_limit,
            quota_limit,
            height,
        );

        let mut tx_hashes = vec![];
        for i in 1..=5 {
            let untx = mock_transaction(100, height + until_block_limit, format!("test{}", i));

            tx_hashes.push(
                Into::<CitaUnverifiedTransaction>::into(untx.clone())
                    .transaction
                    .unwrap()
                    .hash(),
            );
            tx_pool.insert(ctx.clone(), untx).wait().unwrap();
        }
        assert_eq!(tx_pool.tx_cache.len(), 5);

        tx_pool
            .ensure(ctx.clone(), tx_hashes.as_slice())
            .wait()
            .unwrap();
        assert_eq!(tx_pool.callback_cache.len(), 0);
    }

    fn new_test_pool(
        ctx: Context,
        storage: Option<Arc<BlockStorage<MemoryDB>>>,
        size: usize,
        until_block_limit: u64,
        quota_limit: u64,
        height: u64,
    ) -> HashTransactionPool<BlockStorage<MemoryDB>, Secp256k1, BroadcastMock> {
        let secp = Arc::new(Secp256k1::new());

        let storage = storage.unwrap_or_else(|| {
            let db = Arc::new(MemoryDB::new());
            let storage = Arc::new(BlockStorage::new(db));

            let mut block = Block::default();
            block.header.height = height;

            storage.insert_block(ctx.clone(), block).wait().unwrap();
            storage
        });

        let broadcast = BroadcastMock;

        HashTransactionPool::new(
            storage,
            secp,
            broadcast,
            size,
            until_block_limit,
            quota_limit,
            height,
        )
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
        tx.chain_id = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let tx_hash = Into::<CitaTransaction>::into(tx.clone()).hash();
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
        tx.chain_id = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let tx_hash = Into::<CitaTransaction>::into(tx.clone()).hash();

        let signature = secp.sign(&tx_hash, &privkey).unwrap();
        let untx = UnverifiedTransaction {
            transaction: tx,
            signature:   signature.as_bytes().to_vec(),
        };

        SignedTransaction {
            untx:   untx.clone(),
            hash:   tx_hash,
            sender: secp.pubkey_to_address(&pubkey),
        }
    }
}
