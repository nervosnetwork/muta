use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;

use protocol::traits::MixedTxHashes;
use protocol::types::{Hash, SignedTransaction};
use protocol::ProtocolResult;

use crate::map::Map;
use crate::MemPoolError;

/// Wrap `SignedTransaction` with two marks for mempool management.
///
/// Each new transaction inserting into mempool will set `removed` false,
/// while transaction from propose-transaction-sync will additionally set
/// `proposed` true. When shared transaction in `TxCache` removed from map,
/// it will set `removed` true. The `removed` and `proposed` marks will remind
/// queue in `TxCache` to appropriately process elements while packaging
/// transaction hashes for consensus.
pub struct TxWrapper {
    /// Content.
    tx:       SignedTransaction,
    /// While map removes a `shared_tx` during flush, it will mark `removed`
    /// true. Afterwards, queue removes the transaction which marks
    /// `removed` true during package.
    removed:  AtomicBool,
    /// The response transactions in propose-syncing will insert into `TxCache`
    /// marking `proposed` true.
    /// While collecting propose_tx_hashes during package,
    /// it will skips transactions which marks 'proposed` true.
    proposed: AtomicBool,
}

impl TxWrapper {
    #[allow(dead_code)]
    pub(crate) fn new(tx: SignedTransaction) -> Self {
        TxWrapper {
            tx,
            removed: AtomicBool::new(false),
            proposed: AtomicBool::new(false),
        }
    }

    pub(crate) fn propose(tx: SignedTransaction) -> Self {
        TxWrapper {
            tx,
            removed: AtomicBool::new(false),
            proposed: AtomicBool::new(true),
        }
    }

    pub(crate) fn set_removed(&self) {
        self.removed.store(true, Ordering::SeqCst);
    }

    #[inline]
    pub(crate) fn is_removed(&self) -> bool {
        self.removed.load(Ordering::SeqCst)
    }

    #[inline]
    fn is_proposed(&self) -> bool {
        self.proposed.load(Ordering::SeqCst)
    }

    #[inline]
    fn is_timeout(&self, current_height: u64, timeout: u64) -> bool {
        let tx_timeout = self.tx.raw.timeout;
        tx_timeout <= current_height || tx_timeout > timeout
    }
}

/// Share `TxWrapper` for collections in `TxCache`.
pub type SharedTx = Arc<TxWrapper>;

/// An enum stands for package stage
#[derive(PartialEq, Eq)]
enum Stage {
    /// Packing order_tx_hashes
    OrderTxs,
    /// Packing propose_tx_hashes
    ProposeTxs,
    /// Packing finished. Only insert transactions into temp queue.
    Finished,
}

impl Stage {
    fn next(&self) -> Self {
        match self {
            Stage::OrderTxs => Stage::ProposeTxs,
            Stage::ProposeTxs => Stage::Finished,
            Stage::Finished => panic!("There is no next stage after finished stage!"),
        }
    }
}

/// Queue role. Incumbent is for insertion and package.
struct QueueRole {
    incumbent: Arc<ArrayQueue<SharedTx>>,
    candidate: Arc<ArrayQueue<SharedTx>>,
}

/// This is the core structure for caching new transactions and
/// feeding transactions in batch for consensus.
///
/// The queues are served for packaging a batch of transactions in insertion
/// order. The `map` is served for randomly search and removal.
/// All these collections should support concurrent insertion.
/// We set two queues, `queue_0` and `queue_1`, to make package concurrent with
/// insertion. When `queue_0` served for insertion and package begins,
/// transactions pop from `queue_0` and push into `queue_1` while new
/// transactions still insert into `queue_0` concurrently. while `queue_0` pop
/// out, `queue_1` switch to insertion queue.
pub struct TxCache {
    /// One queue.
    queue_0:          Arc<ArrayQueue<SharedTx>>,
    /// Another queue.
    queue_1:          Arc<ArrayQueue<SharedTx>>,
    /// A map for randomly search and removal.
    map:              Map<SharedTx>,
    /// This is used to pick a queue for insertion,
    /// If true selects `queue_0`, else `queue_1`.
    is_zero:          AtomicBool,
    /// This is an atomic state to solve concurrent insertion problem during
    /// package. While switching insertion queues, some transactions may
    /// still insert into the old queue. We use this state to make sure
    /// switch insertions *happen-before* old queue re-pop.
    concurrent_count: AtomicUsize,
}

impl TxCache {
    pub fn new(pool_size: usize) -> Self {
        TxCache {
            queue_0:          Arc::new(ArrayQueue::new(pool_size * 2)),
            queue_1:          Arc::new(ArrayQueue::new(pool_size * 2)),
            map:              Map::new(pool_size * 2),
            is_zero:          AtomicBool::new(true),
            concurrent_count: AtomicUsize::new(0),
        }
    }

    pub async fn len(&self) -> usize {
        self.map.len().await
    }

    pub async fn insert_new_tx(&self, signed_tx: SignedTransaction) -> ProtocolResult<()> {
        let tx_hash = signed_tx.tx_hash.clone();
        let tx_wrapper = TxWrapper::new(signed_tx);
        let shared_tx = Arc::new(tx_wrapper);
        self.insert(tx_hash, shared_tx).await
    }

    pub async fn insert_propose_tx(&self, signed_tx: SignedTransaction) -> ProtocolResult<()> {
        let tx_hash = signed_tx.tx_hash.clone();
        let tx_wrapper = TxWrapper::propose(signed_tx);
        let shared_tx = Arc::new(tx_wrapper);
        self.insert(tx_hash, shared_tx).await
    }

    pub async fn show_unknown(&self, tx_hashes: &[Hash]) -> Vec<Hash> {
        let mut unknow_hashes = vec![];

        for tx_hash in tx_hashes.iter() {
            if !self.contain(&tx_hash).await {
                unknow_hashes.push(tx_hash.clone());
            }
        }

        unknow_hashes
    }

    pub async fn flush(&self, tx_hashes: &[Hash], current_height: u64, timeout: u64) {
        for tx_hash in tx_hashes {
            let opt = self.map.get(tx_hash).await;
            if let Some(shared_tx) = opt {
                shared_tx.set_removed();
            }
        }
        // Dividing set removed and remove into two loops is to avoid lock competition.
        self.map.remove_batch(tx_hashes).await;
        self.flush_incumbent_queue(current_height, timeout).await;
    }

    pub async fn package(
        &self,
        _cycles_limit: u64,
        tx_num_limit: u64,
        current_height: u64,
        timeout: u64,
    ) -> ProtocolResult<MixedTxHashes> {
        let queue_role = self.get_queue_role();

        let mut order_tx_hashes = Vec::new();
        let mut propose_tx_hashes = Vec::new();
        let mut timeout_tx_hashes = Vec::new();

        let mut tx_count: u64 = 0;
        let mut stage = Stage::OrderTxs;

        loop {
            if let Ok(shared_tx) = queue_role.incumbent.pop() {
                let tx_hash = &shared_tx.tx.tx_hash;

                if shared_tx.is_removed() {
                    continue;
                }
                if shared_tx.is_timeout(current_height, timeout) {
                    timeout_tx_hashes.push(tx_hash.clone());
                    continue;
                }
                // After previous filter, tx are valid and should cache in temp_queue.
                if queue_role
                    .candidate
                    .push(Arc::<TxWrapper>::clone(&shared_tx))
                    .is_err()
                {
                    log::error!(
                        "[core_mempool]: candidate queue is full while package, delete {:?}",
                        &shared_tx.tx.tx_hash
                    );
                    self.map.remove(&shared_tx.tx.tx_hash).await;
                }

                if stage == Stage::Finished
                    || (stage == Stage::ProposeTxs && shared_tx.is_proposed())
                {
                    continue;
                }
                tx_count += 1;
                if tx_count > tx_num_limit {
                    stage = stage.next();
                    tx_count = 1;
                }

                match stage {
                    Stage::OrderTxs => order_tx_hashes.push(tx_hash.clone()),
                    Stage::ProposeTxs => propose_tx_hashes.push(tx_hash.clone()),
                    Stage::Finished => {}
                }
            } else {
                // Switch queue_roles
                let new_role = self.switch_queue_role();
                // Transactions may insert into previous incumbent queue during role switch.
                self.process_omission_txs(new_role).await;
                break;
            }
        }
        // Remove timeout tx in map
        self.map.remove_batch(&timeout_tx_hashes).await;

        Ok(MixedTxHashes {
            order_tx_hashes,
            propose_tx_hashes,
        })
    }

    pub async fn check_exist(&self, tx_hash: &Hash) -> ProtocolResult<()> {
        if self.contain(tx_hash).await {
            return Err(MemPoolError::Dup {
                tx_hash: tx_hash.clone(),
            }
            .into());
        }
        Ok(())
    }

    pub async fn check_reach_limit(&self, pool_size: usize) -> ProtocolResult<()> {
        if self.len().await >= pool_size {
            return Err(MemPoolError::ReachLimit { pool_size }.into());
        }
        Ok(())
    }

    pub async fn contain(&self, tx_hash: &Hash) -> bool {
        self.map.contains_key(tx_hash).await
    }

    pub async fn get(&self, tx_hash: &Hash) -> Option<SignedTransaction> {
        self.map
            .get(tx_hash)
            .await
            .map(|shared_tx| shared_tx.tx.clone())
    }

    pub fn queue_len(&self) -> usize {
        if self.is_zero.load(Ordering::Relaxed) {
            self.queue_0.len()
        } else {
            self.queue_1.len()
        }
    }

    async fn insert(&self, tx_hash: Hash, shared_tx: SharedTx) -> ProtocolResult<()> {
        // If multiple transactions exactly the same insert concurrently,
        // this will prevent them to be both insert successfully into queue.
        if self
            .map
            .insert(tx_hash.clone(), Arc::<TxWrapper>::clone(&shared_tx))
            .await
            .is_some()
        {
            return Err(MemPoolError::Dup { tx_hash }.into());
        }

        self.concurrent_count.fetch_add(1, Ordering::SeqCst);
        let rst = self
            .get_queue_role()
            .incumbent
            .push(Arc::<TxWrapper>::clone(&shared_tx));
        self.concurrent_count.fetch_sub(1, Ordering::SeqCst);

        // If queue inserts into queue failed, removes from map.
        if rst.is_err() {
            // If tx_hash exists, it will panic. So repeat check must do before insertion.
            self.map.remove(&tx_hash).await;
            Err(MemPoolError::Insert { tx_hash }.into())
        } else {
            Ok(())
        }
    }

    // Process transactions insert into previous incumbent queue during role switch.
    async fn process_omission_txs(&self, queue_role: QueueRole) {
        'outer: loop {
            // When there are no transaction insertions processing,
            // pop off previous incumbent queue and push them into current incumbent queue.
            if self.concurrent_count.load(Ordering::SeqCst) == 0 {
                while let Ok(shared_tx) = queue_role.candidate.pop() {
                    if queue_role
                        .incumbent
                        .push(Arc::<TxWrapper>::clone(&shared_tx))
                        .is_err()
                    {
                        log::error!(
                            "[core_mempool]: incumbent queue is full while process_omission_txs, delete {:?}",
                            &shared_tx.tx.tx_hash
                        );
                        self.map.remove(&shared_tx.tx.tx_hash).await;
                    }
                }
                break 'outer;
            }
        }
    }

    async fn flush_incumbent_queue(&self, current_height: u64, timeout: u64) {
        let queue_role = self.get_queue_role();
        let mut timeout_tx_hashes = Vec::new();

        loop {
            if let Ok(shared_tx) = queue_role.incumbent.pop() {
                let tx_hash = &shared_tx.tx.tx_hash;

                if shared_tx.is_removed() {
                    continue;
                }
                if shared_tx.is_timeout(current_height, timeout) {
                    timeout_tx_hashes.push(tx_hash.clone());
                    continue;
                }
                // After previous filter, tx are valid and should cache in temp_queue.
                if queue_role
                    .candidate
                    .push(Arc::<TxWrapper>::clone(&shared_tx))
                    .is_err()
                {
                    log::error!(
                        "[core_mempool]: candidate queue is full while flush_incumbent_queue, delete {:?}",
                        &shared_tx.tx.tx_hash
                    );
                    self.map.remove(&shared_tx.tx.tx_hash).await;
                }
            } else {
                // Switch queue_roles
                let new_role = self.switch_queue_role();
                // Transactions may insert into previous incumbent queue during role switch.
                self.process_omission_txs(new_role).await;
                break;
            }
        }
        // Remove timeout tx in map
        self.map.remove_batch(&timeout_tx_hashes).await;
    }

    fn switch_queue_role(&self) -> QueueRole {
        self.is_zero.fetch_xor(true, Ordering::SeqCst);
        self.get_queue_role()
    }

    fn get_queue_role(&self) -> QueueRole {
        let (incumbent, candidate) = if self.is_zero.load(Ordering::SeqCst) {
            (&self.queue_0, &self.queue_1)
        } else {
            (&self.queue_1, &self.queue_0)
        };
        QueueRole {
            incumbent: Arc::clone(incumbent),
            candidate: Arc::clone(candidate),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use std::sync::Arc;

    use rand::random;
    use test::Bencher;

    use protocol::types::{
        Address, Bytes, Hash, RawTransaction, SignedTransaction, TransactionRequest,
    };

    use crate::map::Map;
    use crate::tx_cache::{TxCache, TxWrapper};

    const POOL_SIZE: usize = 1000;
    const BYTES_LEN: usize = 10;
    const TX_NUM: usize = 1000;
    const TX_CYCLE: u64 = 1;
    const TX_NUM_LIMIT: u64 = 20000;
    const CYCLE_LIMIT: u64 = 500;
    const CURRENT_H: u64 = 100;
    const TIMEOUT: u64 = 150;

    fn gen_bytes() -> Vec<u8> {
        (0..BYTES_LEN).map(|_| random::<u8>()).collect()
    }

    fn gen_signed_txs(n: usize) -> Vec<SignedTransaction> {
        let mut vec = Vec::new();
        for _ in 0..n {
            vec.push(mock_signed_tx(gen_bytes()));
        }
        vec
    }

    fn mock_signed_tx(bytes: Vec<u8>) -> SignedTransaction {
        let rand_hash = Hash::digest(Bytes::from(bytes));
        let chain_id = rand_hash.clone();
        let nonce = rand_hash.clone();
        let tx_hash = rand_hash;
        let pubkey = {
            let hex_str = "03380295981e77dcd0a3f50c1d58867e590f2837f03daf639d683ec5e995c02984";
            Bytes::from(hex::decode(hex_str).unwrap())
        };
        let fake_sig = Hash::digest(pubkey.clone()).as_bytes();

        let request = TransactionRequest {
            service_name: "test".to_owned(),
            method:       "test".to_owned(),
            payload:      "test".to_owned(),
        };

        let raw = RawTransaction {
            chain_id,
            nonce,
            timeout: TIMEOUT,
            cycles_limit: TX_CYCLE,
            cycles_price: 1,
            request,
            sender: Address::from_pubkey_bytes(pubkey.clone()).unwrap(),
        };
        SignedTransaction {
            raw,
            tx_hash,
            pubkey,
            signature: fake_sig,
        }
    }

    async fn concurrent_insert(txs: Vec<SignedTransaction>, tx_cache: Arc<TxCache>) {
        let futs = txs
            .into_iter()
            .map(|tx| {
                let tx_cache = Arc::clone(&tx_cache);
                tokio::spawn(async move { tx_cache.insert_new_tx(tx.clone()).await })
            })
            .collect::<Vec<_>>();

        futures::future::try_join_all(futs).await.unwrap();
    }

    async fn concurrent_flush(tx_cache: Arc<TxCache>, tx_hashes: Vec<Hash>, height: u64) {
        tokio::spawn(async move {
            tx_cache.flush(&tx_hashes, height, height + TIMEOUT).await;
        })
        .await
        .unwrap();
    }

    async fn concurrent_package(tx_cache: Arc<TxCache>) {
        tokio::spawn(async move {
            tx_cache
                .package(CYCLE_LIMIT, TX_NUM_LIMIT, CURRENT_H, TIMEOUT)
                .await
                .unwrap();
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_concurrent_insert() {
        let txs = gen_signed_txs(POOL_SIZE / 2);
        let txs: Vec<SignedTransaction> = txs
            .iter()
            .flat_map(|tx| {
                (0..5)
                    .map(|_| tx.clone())
                    .collect::<Vec<SignedTransaction>>()
            })
            .collect();
        let tx_cache = Arc::new(TxCache::new(POOL_SIZE));
        concurrent_insert(txs, Arc::clone(&tx_cache)).await;
        assert_eq!(tx_cache.len().await, POOL_SIZE / 2);
    }

    #[tokio::test]
    async fn test_insert_overlap() {
        let txs = gen_signed_txs(1);
        let tx = txs.get(0).unwrap();
        let map = Map::new(POOL_SIZE);

        let tx_wrapper_0 = TxWrapper::new(tx.clone());
        tx_wrapper_0.set_removed();
        map.insert(tx.tx_hash.clone(), Arc::new(tx_wrapper_0)).await;
        let shared_tx_0 = map.get(&tx.tx_hash).await.unwrap();
        assert!(shared_tx_0.is_removed());

        let tx_wrapper_1 = TxWrapper::new(tx.clone());
        map.insert(tx.tx_hash.clone(), Arc::new(tx_wrapper_1)).await;
        let shared_tx_1 = map.get(&tx.tx_hash).await.unwrap();
        assert!(shared_tx_1.is_removed());
    }

    #[bench]
    fn bench_gen_txs(b: &mut Bencher) {
        b.iter(|| {
            gen_signed_txs(TX_NUM);
        });
    }

    #[bench]
    fn bench_insert(b: &mut Bencher) {
        let mut runtime = tokio::runtime::Runtime::new().unwrap();

        let txs = gen_signed_txs(TX_NUM);
        b.iter(|| {
            let tx_cache = Arc::new(TxCache::new(POOL_SIZE));
            runtime.block_on(concurrent_insert(txs.clone(), Arc::clone(&tx_cache)));
            assert_eq!(runtime.block_on(tx_cache.len()), TX_NUM);
            assert_eq!(tx_cache.queue_len(), TX_NUM);
        });
    }

    #[bench]
    fn bench_flush(b: &mut Bencher) {
        let mut runtime = tokio::runtime::Runtime::new().unwrap();

        let txs = gen_signed_txs(TX_NUM);
        let tx_hashes: Vec<Hash> = txs
            .iter()
            .map(|signed_tx| signed_tx.tx_hash.clone())
            .collect();
        b.iter(|| {
            let tx_cache = Arc::new(TxCache::new(POOL_SIZE));
            runtime.block_on(concurrent_insert(txs.clone(), Arc::clone(&tx_cache)));
            assert_eq!(runtime.block_on(tx_cache.len()), TX_NUM);
            assert_eq!(tx_cache.queue_len(), TX_NUM);
            runtime.block_on(tx_cache.flush(tx_hashes.as_slice(), CURRENT_H, CURRENT_H + TIMEOUT));
            assert_eq!(runtime.block_on(tx_cache.len()), 0);
            assert_eq!(tx_cache.queue_len(), 0);
        });
    }

    #[bench]
    fn bench_flush_insert(b: &mut Bencher) {
        let mut runtime = tokio::runtime::Runtime::new().unwrap();

        let txs_base = gen_signed_txs(TX_NUM / 2);
        let txs_insert = gen_signed_txs(TX_NUM / 2);
        let txs_flush: Vec<Hash> = txs_base
            .iter()
            .map(|signed_tx| signed_tx.tx_hash.clone())
            .collect();
        b.iter(|| {
            let tx_cache = Arc::new(TxCache::new(POOL_SIZE));
            runtime.block_on(concurrent_insert(txs_base.clone(), Arc::clone(&tx_cache)));
            runtime.block_on(concurrent_flush(
                Arc::clone(&tx_cache),
                txs_flush.clone(),
                CURRENT_H,
            ));
            runtime.block_on(concurrent_insert(txs_insert.clone(), Arc::clone(&tx_cache)));
            assert_eq!(runtime.block_on(tx_cache.len()), TX_NUM / 2);
            assert_eq!(tx_cache.queue_len(), TX_NUM / 2);
        });
    }

    #[bench]
    fn bench_package(b: &mut Bencher) {
        let mut runtime = tokio::runtime::Runtime::new().unwrap();

        let txs = gen_signed_txs(TX_NUM);
        let tx_cache = Arc::new(TxCache::new(POOL_SIZE));
        runtime.block_on(concurrent_insert(txs, Arc::clone(&tx_cache)));
        b.iter(|| {
            let mixed_tx_hashes = runtime
                .block_on(tx_cache.package(TX_NUM_LIMIT, CYCLE_LIMIT, CURRENT_H, TIMEOUT))
                .unwrap();
            assert_eq!(
                mixed_tx_hashes.order_tx_hashes.len(),
                (CYCLE_LIMIT / TX_CYCLE) as usize
            );
        });
    }

    #[bench]
    fn bench_package_insert(b: &mut Bencher) {
        let mut runtime = tokio::runtime::Runtime::new().unwrap();

        let txs = gen_signed_txs(TX_NUM / 2);
        let txs_insert = gen_signed_txs(TX_NUM / 2);
        b.iter(|| {
            let tx_cache = Arc::new(TxCache::new(POOL_SIZE));
            runtime.block_on(concurrent_insert(txs.clone(), Arc::clone(&tx_cache)));
            runtime.block_on(concurrent_package(Arc::clone(&tx_cache)));
            runtime.block_on(concurrent_insert(txs_insert.clone(), Arc::clone(&tx_cache)));
            assert_eq!(runtime.block_on(tx_cache.len()), TX_NUM);
            assert_eq!(tx_cache.queue_len(), TX_NUM);
        });
    }
}
