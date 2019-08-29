use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam_queue::ArrayQueue;

use protocol::traits::MixedTxHashes;
use protocol::types::{Hash, SignedTransaction};
use protocol::ProtocolResult;

use crate::error::MemPoolError;
use crate::map::Map;

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
    tx: SignedTransaction,
    /// While map removes a `shared_tx` during flush, it will mark `removed`
    /// true. Afterwards, queue removes the transaction which marks
    /// `removed` true during package.
    removed: AtomicBool,
    /// The response transactions in propose-syncing will insert into `TxCache`
    /// marking `proposed` true.
    /// While collecting propose_tx_hashes during package,
    /// it will skips transactions which marks 'proposed` true.
    proposed: AtomicBool,
}

impl TxWrapper {
    #[allow(dead_code)]
    fn new(tx: SignedTransaction) -> Self {
        TxWrapper {
            tx,
            removed: AtomicBool::new(false),
            proposed: AtomicBool::new(false),
        }
    }

    fn propose(tx: SignedTransaction) -> Self {
        TxWrapper {
            tx,
            removed: AtomicBool::new(false),
            proposed: AtomicBool::new(true),
        }
    }

    fn set_removed(&self) {
        self.removed.store(true, Ordering::SeqCst);
    }

    #[inline]
    fn is_removed(&self) -> bool {
        self.removed.load(Ordering::SeqCst)
    }

    #[inline]
    fn is_proposed(&self) -> bool {
        self.proposed.load(Ordering::SeqCst)
    }

    #[inline]
    fn is_timeout(&self, current_epoch_id: u64, timeout: u64) -> bool {
        let tx_timeout = self.tx.raw.timeout;
        tx_timeout <= current_epoch_id || tx_timeout > timeout
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
struct QueueRole<'a> {
    incumbent: &'a ArrayQueue<SharedTx>,
    candidate: &'a ArrayQueue<SharedTx>,
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
    queue_0: ArrayQueue<SharedTx>,
    /// Another queue.
    queue_1: ArrayQueue<SharedTx>,
    /// A map for randomly search and removal.
    map: Map<SharedTx>,
    /// This is used to pick a queue for insertion,
    /// If true selects `queue_0`, else `queue_1`.
    is_zero: AtomicBool,
    /// This is an atomic state to solve concurrent insertion problem during
    /// package. While switching insertion queues, some transactions may
    /// still insert into the old queue. We use this state to make sure
    /// switch insertions *happen-before* old queue re-pop.
    concurrent_count: AtomicUsize,
}

impl TxCache {
    pub fn new(pool_size: usize) -> Self {
        TxCache {
            queue_0:          ArrayQueue::new(pool_size),
            queue_1:          ArrayQueue::new(pool_size),
            map:              Map::new(pool_size),
            is_zero:          AtomicBool::new(true),
            concurrent_count: AtomicUsize::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn insert_new_tx(&self, signed_tx: SignedTransaction) -> ProtocolResult<()> {
        let tx_hash = signed_tx.tx_hash.clone();
        let tx_wrapper = TxWrapper::new(signed_tx.clone());
        let shared_tx = Arc::new(tx_wrapper);
        self.insert(tx_hash, shared_tx)
    }

    pub fn insert_propose_tx(&self, signed_tx: SignedTransaction) -> ProtocolResult<()> {
        let tx_hash = signed_tx.tx_hash.clone();
        let tx_wrapper = TxWrapper::propose(signed_tx.clone());
        let shared_tx = Arc::new(tx_wrapper);
        self.insert(tx_hash, shared_tx)
    }

    pub fn show_unknown(&self, tx_hashes: Vec<Hash>) -> Vec<Hash> {
        tx_hashes
            .into_iter()
            .filter(|tx_hash| self.contain(tx_hash))
            .collect()
    }

    pub fn flush(&self, tx_hashes: &[Hash]) {
        for tx_hash in tx_hashes {
            let opt = self.map.get(tx_hash);
            if let Some(shared_tx) = opt {
                shared_tx.set_removed();
            }
        }
        // Dividing set removed and remove into two loops is to avoid lock competition.
        self.map.deletes(tx_hashes);
    }

    pub fn package(
        &self,
        cycle_limit: u64,
        current_epoch_id: u64,
        timeout: u64,
    ) -> ProtocolResult<MixedTxHashes> {
        let queue_role = self.get_queue_role();

        let mut order_tx_hashes = Vec::new();
        let mut propose_tx_hashes = Vec::new();
        let mut timeout_tx_hashes = Vec::new();

        let mut cycle_count: u64 = 0;
        let mut stage = Stage::OrderTxs;

        loop {
            if let Ok(shared_tx) = queue_role.incumbent.pop() {
                let tx_hash = &shared_tx.tx.tx_hash;

                if shared_tx.is_removed() {
                    continue;
                }
                if shared_tx.is_timeout(current_epoch_id, timeout) {
                    timeout_tx_hashes.push(tx_hash.clone());
                    continue;
                }
                // After previous filter, tx are valid and should cache in temp_queue.
                queue_role
                    .candidate
                    .push(Arc::<TxWrapper>::clone(&shared_tx))
                    .map_err(|_| MemPoolError::InsertCandidate {
                        len: queue_role.candidate.len(),
                    })?;

                if stage == Stage::Finished
                    || (stage == Stage::ProposeTxs && shared_tx.is_proposed())
                {
                    continue;
                }
                // Accumulate cycles. The order_tx_hashes and the propose_tx_hashes both collect
                // transactions under cycle limit.
                cycle_count += shared_tx.tx.raw.fee.cycle;
                if cycle_count > cycle_limit {
                    stage = stage.next();
                    cycle_count = 0;
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
                self.process_omission_txs(new_role);
                break;
            }
        }
        // Remove timeout tx in map
        self.map.deletes(&timeout_tx_hashes);

        Ok(MixedTxHashes {
            order_tx_hashes,
            propose_tx_hashes,
        })
    }

    #[inline]
    pub fn contain(&self, tx_hash: &Hash) -> bool {
        self.map.contains_key(tx_hash)
    }

    #[inline]
    pub fn get(&self, tx_hash: &Hash) -> Option<SignedTransaction> {
        self.map.get(tx_hash).map(|shared_tx| shared_tx.tx.clone())
    }

    #[allow(dead_code)]
    fn queue_len(&self) -> usize {
        if self.is_zero.load(Ordering::Relaxed) {
            self.queue_0.len()
        } else {
            self.queue_1.len()
        }
    }

    fn insert(&self, tx_hash: Hash, shared_tx: SharedTx) -> ProtocolResult<()> {
        // If multiple transactions exactly the same insert concurrently,
        // this will prevent them to be both insert successfully into queue.
        if self
            .map
            .insert(tx_hash.clone(), Arc::<TxWrapper>::clone(&shared_tx))
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
            self.map.remove(&tx_hash);
            Err(MemPoolError::Insert { tx_hash }.into())
        } else {
            Ok(())
        }
    }

    // Process transactions insert into previous incumbent queue during role switch.
    fn process_omission_txs(&self, queue_role: QueueRole) {
        'outer: loop {
            // When there are no transaction insertions processing,
            // pop off previous incumbent queue and push them into current incumbent queue.
            if self.concurrent_count.load(Ordering::SeqCst) == 0 {
                while let Ok(shared_tx) = queue_role.candidate.pop() {
                    let _ = queue_role
                        .incumbent
                        .push(Arc::<TxWrapper>::clone(&shared_tx));
                }
                break 'outer;
            }
        }
    }

    fn switch_queue_role(&self) -> QueueRole {
        self.is_zero.fetch_xor(true, Ordering::SeqCst);
        self.get_queue_role()
    }

    #[inline]
    fn get_queue_role(&self) -> QueueRole {
        let (incumbent, candidate) = if self.is_zero.load(Ordering::SeqCst) {
            (&self.queue_0, &self.queue_1)
        } else {
            (&self.queue_1, &self.queue_0)
        };
        QueueRole {
            incumbent,
            candidate,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use std::sync::Arc;
    use std::thread;

    use bytes::Bytes;
    use num_traits::FromPrimitive;
    use rand::random;
    use rayon::iter::IntoParallelRefIterator;
    use rayon::prelude::*;
    use test::Bencher;

    use protocol::types::{
        AccountAddress, Fee, Hash, RawTransaction, SignedTransaction, TransactionAction,
    };

    use crate::tx_cache::TxCache;
    use std::thread::JoinHandle;

    const POOL_SIZE: usize = 100_000;
    const BYTES_LEN: usize = 10;
    const TX_NUM: usize = 100_000;
    const TX_CYCLE: u64 = 1;
    const CYCLE_LIMIT: u64 = 50000;
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
        let asset_id = rand_hash.clone();
        let nonce = rand_hash.clone();
        let tx_hash = rand_hash.clone();
        let add_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
        let bytes = Bytes::from(hex::decode(add_str).unwrap());
        let address = AccountAddress::from_bytes(bytes.clone()).unwrap();
        let fee = Fee {
            asset_id: asset_id.clone(),
            cycle:    TX_CYCLE,
        };
        let action = TransactionAction::Transfer {
            receiver: address.clone(),
            asset_id,
            amount: FromPrimitive::from_i32(10_000).unwrap(),
        };
        let raw = RawTransaction {
            chain_id,
            nonce,
            timeout: TIMEOUT,
            fee,
            action,
        };
        SignedTransaction {
            raw,
            tx_hash,
            pubkey: bytes.clone(),
            signature: bytes,
        }
    }

    fn concurrent_insert(txs: Vec<SignedTransaction>, tx_cache: &TxCache) {
        txs.par_iter().for_each(|signed_tx| {
            tx_cache.insert_new_tx(signed_tx.clone()).unwrap();
        });
    }

    fn concurrent_flush(tx_cache: &Arc<TxCache>, tx_hashes: Vec<Hash>) -> JoinHandle<()> {
        let tx_cache_clone = Arc::<TxCache>::clone(tx_cache);
        let tx_hashes = tx_hashes.clone();
        thread::spawn(move || {
            tx_cache_clone.flush(&tx_hashes);
        })
    }

    fn concurrent_package(tx_cache: &Arc<TxCache>) -> JoinHandle<()> {
        let tx_cache_clone = Arc::<TxCache>::clone(tx_cache);
        thread::spawn(move || {
            tx_cache_clone
                .package(CYCLE_LIMIT, CURRENT_H, TIMEOUT)
                .unwrap();
        })
    }

    #[bench]
    fn bench_gen_txs(b: &mut Bencher) {
        b.iter(|| {
            gen_signed_txs(TX_NUM);
        });
    }

    #[bench]
    fn bench_insert(b: &mut Bencher) {
        let txs = gen_signed_txs(TX_NUM);
        b.iter(|| {
            let tx_cache = TxCache::new(POOL_SIZE);
            concurrent_insert(txs.clone(), &tx_cache);
            assert_eq!(tx_cache.len(), TX_NUM);
            assert_eq!(tx_cache.queue_len(), TX_NUM);
        });
    }

    #[bench]
    fn bench_flush(b: &mut Bencher) {
        let txs = gen_signed_txs(TX_NUM);
        let tx_hashes: Vec<Hash> = txs
            .iter()
            .map(|signed_tx| signed_tx.tx_hash.clone())
            .collect();
        b.iter(|| {
            let tx_cache = TxCache::new(POOL_SIZE);
            concurrent_insert(txs.clone(), &tx_cache);
            assert_eq!(tx_cache.len(), TX_NUM);
            assert_eq!(tx_cache.queue_len(), TX_NUM);
            tx_cache.flush(tx_hashes.as_slice());
            assert_eq!(tx_cache.len(), 0);
            assert_eq!(tx_cache.queue_len(), TX_NUM);
        });
    }

    #[bench]
    fn bench_flush_insert(b: &mut Bencher) {
        let txs_base = gen_signed_txs(TX_NUM / 2);
        let txs_insert = gen_signed_txs(TX_NUM / 2);
        let txs_flush: Vec<Hash> = txs_base
            .iter()
            .map(|signed_tx| signed_tx.tx_hash.clone())
            .collect();
        b.iter(|| {
            let tx_cache = Arc::new(TxCache::new(POOL_SIZE));
            concurrent_insert(txs_base.clone(), &tx_cache);
            let handle = concurrent_flush(&tx_cache, txs_flush.clone());
            concurrent_insert(txs_insert.clone(), &tx_cache);
            handle.join().unwrap();
            assert_eq!(tx_cache.len(), TX_NUM / 2);
            assert_eq!(tx_cache.queue_len(), TX_NUM);
        });
    }

    #[bench]
    fn bench_package(b: &mut Bencher) {
        let txs = gen_signed_txs(TX_NUM);
        let tx_cache = TxCache::new(POOL_SIZE);
        concurrent_insert(txs.clone(), &tx_cache);
        b.iter(|| {
            let mixed_tx_hashes = tx_cache.package(CYCLE_LIMIT, CURRENT_H, TIMEOUT).unwrap();
            assert_eq!(
                mixed_tx_hashes.order_tx_hashes.len(),
                (CYCLE_LIMIT / TX_CYCLE) as usize
            );
        });
    }

    #[bench]
    fn bench_package_insert(b: &mut Bencher) {
        let txs = gen_signed_txs(TX_NUM / 2);
        let txs_insert = gen_signed_txs(TX_NUM / 2);
        b.iter(|| {
            let tx_cache = Arc::new(TxCache::new(POOL_SIZE));
            concurrent_insert(txs.clone(), &tx_cache);
            let handle = concurrent_package(&tx_cache);
            concurrent_insert(txs_insert.clone(), &tx_cache);
            handle.join().unwrap();
            assert_eq!(tx_cache.len(), TX_NUM);
            assert_eq!(tx_cache.queue_len(), TX_NUM);
        });
    }
}
