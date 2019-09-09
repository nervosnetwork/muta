#[cfg(test)]
mod tests {
    extern crate test;

    use std::collections::HashMap;
    use std::convert::{From, TryFrom};
    use std::sync::Arc;

    use async_trait::async_trait;
    use bytes::Bytes;
    use chashmap::CHashMap;
    use futures::executor;
    use num_traits::FromPrimitive;
    use rand::random;
    use rand::rngs::OsRng;
    use rayon::iter::IntoParallelRefIterator;
    use rayon::prelude::*;
    use test::Bencher;

    use common_crypto::{
        Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey, Secp256k1PublicKey,
        Secp256k1Signature, Signature,
    };
    use protocol::codec::ProtocolCodec;
    use protocol::traits::{Context, MemPool, MemPoolAdapter, MixedTxHashes};
    use protocol::types::{
        AccountAddress as Address, Fee, Hash, RawTransaction, SignedTransaction, TransactionAction,
    };
    use protocol::ProtocolResult;

    use crate::{HashMemPool, MemPoolError};

    const AMOUNT: i32 = 42;
    const CYCLE_LIMIT: u64 = 10_000;
    const CURRENT_EPOCH_ID: u64 = 999;
    const POOL_SIZE: usize = 100_000;
    const TIMEOUT: u64 = 1000;
    const TIMEOUT_GAP: u64 = 100;
    const TX_CYCLE: u64 = 1;

    pub struct HashMemPoolAdapter {
        network_txs: CHashMap<Hash, SignedTransaction>,
    }

    impl HashMemPoolAdapter {
        fn new() -> HashMemPoolAdapter {
            HashMemPoolAdapter {
                network_txs: CHashMap::new(),
            }
        }
    }

    #[async_trait]
    impl MemPoolAdapter for HashMemPoolAdapter {
        async fn pull_txs(
            &self,
            _ctx: Context,
            tx_hashes: Vec<Hash>,
        ) -> ProtocolResult<Vec<SignedTransaction>> {
            let mut vec = Vec::new();
            for hash in tx_hashes {
                if let Some(tx) = self.network_txs.get(&hash) {
                    vec.push(tx.clone());
                }
            }
            Ok(vec)
        }

        async fn broadcast_tx(&self, _ctx: Context, tx: SignedTransaction) -> ProtocolResult<()> {
            self.network_txs.insert(tx.tx_hash.clone(), tx);
            Ok(())
        }

        async fn check_signature(
            &self,
            _ctx: Context,
            tx: SignedTransaction,
        ) -> ProtocolResult<()> {
            check_hash(tx.clone()).await?;
            check_sig(&tx)
        }

        async fn check_transaction(
            &self,
            _ctx: Context,
            _tx: SignedTransaction,
        ) -> ProtocolResult<()> {
            Ok(())
        }

        async fn check_storage_exist(&self, _ctx: Context, _tx_hash: Hash) -> ProtocolResult<()> {
            Ok(())
        }
    }

    macro_rules! insert {
        (normal($pool_size: expr, $input: expr, $output: expr)) => {
            insert!(inner($pool_size, 1, $input, 0, $output));
        };
        (repeat($repeat: expr, $input: expr, $output: expr)) => {
            insert!(inner($input * 10, $repeat, $input, 0, $output));
        };
        (invalid($valid: expr, $invalid: expr, $output: expr)) => {
            insert!(inner($valid * 10, 1, $valid, $invalid, $output));
        };
        (inner($pool_size: expr, $repeat: expr, $valid: expr, $invalid: expr, $output: expr)) => {
            let mempool = Arc::new(new_mempool(
                $pool_size,
                CYCLE_LIMIT,
                TIMEOUT_GAP,
                CURRENT_EPOCH_ID,
            ));
            let txs = mock_txs($valid, $invalid, TIMEOUT);
            for _ in 0..$repeat {
                concurrent_insert(txs.clone(), Arc::clone(&mempool));
            }
            assert_eq!(mempool.get_tx_cache().len(), $output);
        };
    }

    #[test]
    fn test_insert() {
        // 1. insertion under pool size.
        insert!(normal(100, 100, 100));

        // 2. insertion above pool size.
        insert!(normal(100, 101, 100));

        // 3. repeat insertion
        insert!(repeat(5, 200, 200));

        // 4. invalid insertion
        insert!(invalid(80, 10, 80));
    }

    macro_rules! package {
        (normal($cycle_limit: expr, $insert: expr, $expect_order: expr, $expect_propose: expr)) => {
            package!(inner(
                $cycle_limit,
                CURRENT_EPOCH_ID,
                TIMEOUT_GAP,
                TIMEOUT,
                $insert,
                $expect_order,
                $expect_propose
            ));
        };
        (timeout($current_epoch_id: expr, $timeout_gap: expr, $timeout: expr, $insert: expr, $expect: expr)) => {
            package!(inner(
                $insert,
                $current_epoch_id,
                $timeout_gap,
                $timeout,
                $insert,
                $expect,
                0
            ));
        };
        (inner($cycle_limit: expr, $current_epoch_id: expr, $timeout_gap: expr, $timeout: expr, $insert: expr, $expect_order: expr, $expect_propose: expr)) => {
            let mempool = &Arc::new(new_mempool(
                $insert * 10,
                $cycle_limit,
                $timeout_gap,
                $current_epoch_id,
            ));
            let txs = mock_txs($insert, 0, $timeout);
            concurrent_insert(txs.clone(), Arc::clone(mempool));
            let mixed_tx_hashes = exec_package(Arc::clone(mempool));
            assert_eq!(mixed_tx_hashes.order_tx_hashes.len(), $expect_order);
            assert_eq!(mixed_tx_hashes.propose_tx_hashes.len(), $expect_propose);
        };
    }

    #[test]
    fn test_package() {
        // 1. pool_size <= cycle_limit
        package!(normal(100, 50, 50, 0));
        package!(normal(100, 100, 100, 0));

        // 2. cycle_limit < pool_size <= 2 * cycle_limit
        package!(normal(100, 101, 100, 1));
        package!(normal(100, 200, 100, 100));

        // 3. 2 * cycle_limit < pool_size
        package!(normal(100, 201, 100, 100));

        // 4. current_epoch_id >= tx.timeout
        package!(timeout(100, 50, 100, 10, 0));
        package!(timeout(100, 50, 90, 10, 0));

        // 5. current_epoch_id + timeout_gap < tx.timeout
        package!(timeout(100, 50, 151, 10, 0));
        package!(timeout(100, 50, 160, 10, 0));

        // 6. tx.timeout - timeout_gap =< current_epoch_id < tx.timeout
        package!(timeout(100, 50, 150, 10, 10));
        package!(timeout(100, 50, 101, 10, 10));
    }

    #[test]
    fn test_package_order_consistent_with_insert_order() {
        let mempool = &Arc::new(default_mempool());

        let txs = &default_mock_txs(100);
        txs.iter()
            .for_each(|signed_tx| exec_insert(signed_tx, Arc::clone(mempool)));
        let mixed_tx_hashes = exec_package(Arc::clone(mempool));
        assert!(check_order_consistant(&mixed_tx_hashes, txs));

        // flush partial txs and test order consistency
        let (remove_txs, reserve_txs) = txs.split_at(50);
        let remove_hashes: Vec<Hash> = remove_txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        exec_flush(remove_hashes, Arc::clone(mempool));
        let mixed_tx_hashes = exec_package(Arc::clone(mempool));
        assert!(check_order_consistant(&mixed_tx_hashes, reserve_txs));
    }

    #[test]
    fn test_flush() {
        let mempool = Arc::new(default_mempool());

        // insert txs
        let txs = default_mock_txs(555);
        concurrent_insert(txs.clone(), Arc::clone(&mempool));
        assert_eq!(mempool.get_tx_cache().len(), 555);

        let callback_cache = mempool.get_callback_cache();
        txs.iter().for_each(|tx| {
            callback_cache.insert(tx.tx_hash.clone(), tx.clone());
        });
        assert_eq!(callback_cache.len(), 555);

        // flush exist txs
        let (remove_txs, _) = txs.split_at(123);
        let remove_hashes: Vec<Hash> = remove_txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        exec_flush(remove_hashes, Arc::clone(&mempool));
        assert_eq!(mempool.get_tx_cache().len(), 432);
        assert_eq!(mempool.get_tx_cache().queue_len(), 555);
        exec_package(Arc::clone(&mempool));
        assert_eq!(mempool.get_tx_cache().queue_len(), 432);
        assert_eq!(callback_cache.len(), 0);

        // flush absent txs
        let txs = default_mock_txs(222);
        let remove_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        exec_flush(remove_hashes, Arc::clone(&mempool));
        assert_eq!(mempool.get_tx_cache().len(), 432);
        assert_eq!(mempool.get_tx_cache().queue_len(), 432);
    }

    macro_rules! ensure_order_txs {
        ($in_pool: expr, $out_pool: expr) => {
            let mempool = &Arc::new(default_mempool());

            let txs = &default_mock_txs($in_pool + $out_pool);
            let (in_pool_txs, out_pool_txs) = txs.split_at($in_pool);
            concurrent_insert(in_pool_txs.to_vec(), Arc::clone(mempool));
            concurrent_broadcast(out_pool_txs.to_vec(), Arc::clone(mempool));

            let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
            exec_ensure_order_txs(tx_hashes.clone(), Arc::clone(mempool));

            assert_eq!(mempool.get_callback_cache().len(), $out_pool);

            let fetch_txs = exec_get_full_txs(tx_hashes, Arc::clone(mempool));
            assert_eq!(fetch_txs.len(), txs.len());
        };
    }

    #[test]
    fn test_ensure_order_txs() {
        // all txs are in pool
        ensure_order_txs!(100, 0);
        // 50 txs are not in pool
        ensure_order_txs!(50, 50);
        // all txs are not in pool
        ensure_order_txs!(0, 100);
    }

    #[test]
    fn test_sync_propose_txs() {
        let mempool = &Arc::new(default_mempool());

        let txs = &default_mock_txs(50);
        let (exist_txs, need_sync_txs) = txs.split_at(20);
        concurrent_insert(exist_txs.to_vec(), Arc::clone(mempool));
        concurrent_broadcast(need_sync_txs.to_vec(), Arc::clone(mempool));

        let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        exec_sync_propose_txs(tx_hashes.clone(), Arc::clone(mempool));

        assert_eq!(mempool.get_tx_cache().len(), 50);
    }

    #[bench]
    fn bench_insert(b: &mut Bencher) {
        let mempool = &Arc::new(default_mempool());

        b.iter(|| {
            let txs = default_mock_txs(100);
            concurrent_insert(txs, Arc::clone(mempool));
        });
    }

    #[bench]
    fn bench_package(b: &mut Bencher) {
        let mempool = Arc::new(default_mempool());
        let txs = default_mock_txs(50_000);
        concurrent_insert(txs.clone(), Arc::clone(&mempool));
        b.iter(|| {
            exec_package(Arc::clone(&mempool));
        });
    }

    #[bench]
    fn bench_flush(b: &mut Bencher) {
        let mempool = &Arc::new(default_mempool());
        let txs = &default_mock_txs(100);
        let remove_hashes: &Vec<Hash> = &txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        b.iter(|| {
            concurrent_insert(txs.clone(), Arc::clone(mempool));
            exec_flush(remove_hashes.clone(), Arc::clone(mempool));
            exec_package(Arc::clone(mempool));
        });
    }

    #[bench]
    fn bench_mock_txs(b: &mut Bencher) {
        b.iter(|| {
            default_mock_txs(100);
        });
    }

    #[bench]
    fn bench_check_sig(b: &mut Bencher) {
        let txs = &default_mock_txs(100);

        b.iter(|| {
            concurrent_check_sig(txs.clone());
        });
    }

    fn default_mock_txs(size: usize) -> Vec<SignedTransaction> {
        mock_txs(size, 0, TIMEOUT)
    }

    fn mock_txs(valid_size: usize, invalid_size: usize, timeout: u64) -> Vec<SignedTransaction> {
        let mut vec = Vec::new();
        let mut rng = OsRng::new().expect("OsRng");
        let (priv_key, pub_key) = Secp256k1::generate_keypair(&mut rng);
        let address = pub_key_to_address(&pub_key).unwrap();
        for i in 0..valid_size + invalid_size {
            vec.push(mock_signed_tx(
                &priv_key,
                &pub_key,
                &address,
                timeout,
                i < valid_size,
            ));
        }
        vec
    }

    fn default_mempool() -> HashMemPool<HashMemPoolAdapter> {
        new_mempool(POOL_SIZE, CYCLE_LIMIT, TIMEOUT_GAP, CURRENT_EPOCH_ID)
    }

    fn new_mempool(
        pool_size: usize,
        cycle_limit: u64,
        timeout_gap: u64,
        current_epoch_id: u64,
    ) -> HashMemPool<HashMemPoolAdapter> {
        let adapter = HashMemPoolAdapter::new();
        HashMemPool::new(
            pool_size,
            timeout_gap,
            cycle_limit,
            current_epoch_id,
            adapter,
        )
    }

    fn pub_key_to_address(pub_key: &Secp256k1PublicKey) -> ProtocolResult<Address> {
        let mut pub_key_str = Hash::digest(pub_key.to_bytes()).as_hex();
        pub_key_str.truncate(40);
        pub_key_str.insert_str(0, "10");
        Address::from_bytes(Bytes::from(hex::decode(pub_key_str).unwrap()))
    }

    async fn check_hash(tx: SignedTransaction) -> ProtocolResult<()> {
        let mut raw = tx.raw;
        let raw_bytes = raw.encode().await?;
        let tx_hash = Hash::digest(raw_bytes);
        if tx_hash != tx.tx_hash {
            return Err(MemPoolError::CheckHash {
                expect: tx.tx_hash.clone(),
                actual: tx_hash.clone(),
            }
            .into());
        }
        Ok(())
    }

    fn check_sig(tx: &SignedTransaction) -> ProtocolResult<()> {
        if Secp256k1::verify_signature(&tx.tx_hash.as_bytes(), &tx.signature, &tx.pubkey).is_err() {
            return Err(MemPoolError::CheckSig {
                tx_hash: tx.tx_hash.clone(),
            }
            .into());
        }
        Ok(())
    }

    fn concurrent_check_sig(txs: Vec<SignedTransaction>) {
        txs.par_iter().for_each(|signed_tx| {
            check_sig(signed_tx).unwrap();
        });
    }

    fn concurrent_insert(
        txs: Vec<SignedTransaction>,
        mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
    ) {
        txs.par_iter()
            .for_each(|signed_tx| exec_insert(signed_tx, Arc::clone(&mempool)));
    }

    fn concurrent_broadcast(
        txs: Vec<SignedTransaction>,
        mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
    ) {
        txs.par_iter().for_each(|signed_tx| {
            executor::block_on(async {
                mempool
                    .get_adapter()
                    .broadcast_tx(HashMap::new(), signed_tx.clone())
                    .await
                    .unwrap();
            })
        });
    }

    fn exec_insert(signed_tx: &SignedTransaction, mempool: Arc<HashMemPool<HashMemPoolAdapter>>) {
        executor::block_on(async {
            let _ = mempool.insert(HashMap::new(), signed_tx.clone()).await;
        });
    }

    fn exec_flush(remove_hashes: Vec<Hash>, mempool: Arc<HashMemPool<HashMemPoolAdapter>>) {
        executor::block_on(async {
            mempool.flush(HashMap::new(), remove_hashes).await.unwrap();
        });
    }

    fn exec_package(mempool: Arc<HashMemPool<HashMemPoolAdapter>>) -> MixedTxHashes {
        executor::block_on(async { mempool.package(HashMap::new()).await.unwrap() })
    }

    fn exec_ensure_order_txs(
        require_hashes: Vec<Hash>,
        mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
    ) {
        executor::block_on(async {
            mempool
                .ensure_order_txs(HashMap::new(), require_hashes)
                .await
                .unwrap();
        })
    }

    fn exec_sync_propose_txs(
        require_hashes: Vec<Hash>,
        mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
    ) {
        executor::block_on(async {
            mempool
                .sync_propose_txs(HashMap::new(), require_hashes)
                .await
                .unwrap();
        })
    }

    fn exec_get_full_txs(
        require_hashes: Vec<Hash>,
        mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
    ) -> Vec<SignedTransaction> {
        executor::block_on(async {
            mempool
                .get_full_txs(HashMap::new(), require_hashes)
                .await
                .unwrap()
        })
    }

    fn mock_signed_tx(
        priv_key: &Secp256k1PrivateKey,
        pub_key: &Secp256k1PublicKey,
        address: &Address,
        timeout: u64,
        valid: bool,
    ) -> SignedTransaction {
        let nonce = Hash::digest(Bytes::from(get_random_bytes(10)));
        let fee = Fee {
            asset_id: nonce.clone(),
            cycle:    TX_CYCLE,
        };
        let action = TransactionAction::Transfer {
            receiver: address.clone(),
            asset_id: nonce.clone(),
            amount:   FromPrimitive::from_i32(AMOUNT).unwrap(),
        };
        let mut raw = RawTransaction {
            chain_id: nonce.clone(),
            nonce,
            timeout,
            fee,
            action,
        };

        let raw_bytes = executor::block_on(async { raw.encode().await.unwrap() });
        let tx_hash = Hash::digest(raw_bytes);

        let signature = if valid {
            Secp256k1::sign_message(&tx_hash.as_bytes(), &priv_key.to_bytes()).unwrap()
        } else {
            Secp256k1Signature::try_from([0u8; 64].as_parallel_slice()).unwrap()
        };

        SignedTransaction {
            raw,
            tx_hash,
            pubkey: pub_key.to_bytes(),
            signature: signature.to_bytes(),
        }
    }

    fn get_random_bytes(len: usize) -> Vec<u8> {
        (0..len).map(|_| random::<u8>()).collect()
    }

    fn check_order_consistant(mixed_tx_hashes: &MixedTxHashes, txs: &[SignedTransaction]) -> bool {
        mixed_tx_hashes
            .order_tx_hashes
            .iter()
            .enumerate()
            .any(|(i, hash)| hash == &txs.get(i).unwrap().tx_hash)
    }
}
