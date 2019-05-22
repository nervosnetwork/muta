use parking_lot::RwLock;
use rayon::prelude::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use std::collections::HashMap;

use core_types::{Hash, SignedTransaction};

/// The "TxCache" is a transaction pool cache that uses 16 buckets to
/// decentralize store transactions.
/// Why use 16 buckets? We take 0 bytes of each "tx_hash" and shift it 4 bits to
/// the left to get a number in the range 0~15, which corresponds to 16 buckets.
pub struct TxCache {
    buckets: Vec<Bucket>,
}

impl TxCache {
    pub fn new(cache_size: usize) -> Self {
        let mut buckets = Vec::with_capacity(16);
        for _ in 0..16 {
            buckets.push(Bucket {
                // Allocate enough space to avoid triggering resize.
                store: RwLock::new(HashMap::with_capacity(cache_size)),
            });
        }
        Self { buckets }
    }

    pub fn insert(&self, tx: SignedTransaction) {
        let bucket = self.get_bucket(&tx.hash);
        bucket.insert(tx)
    }

    // TODO: concurrently insert
    pub fn insert_batch(&self, txs: Vec<SignedTransaction>) {
        let mut h: HashMap<usize, Vec<SignedTransaction>> = HashMap::new();

        for tx in txs.into_iter() {
            let index = get_index(&tx.hash);
            h.entry(index).or_insert_with(|| vec![]).push(tx);
        }

        for (index, txs) in h.into_iter() {
            self.buckets[index].insert_batch(txs);
        }
    }

    pub fn contains_key(&self, tx_hash: &Hash) -> bool {
        let bucket = self.get_bucket(tx_hash);
        bucket.contains_key(tx_hash)
    }

    pub fn get(&self, tx_hash: &Hash) -> Option<SignedTransaction> {
        let bucket = self.get_bucket(tx_hash);
        bucket.get(tx_hash)
    }

    // TODO: concurrently get
    pub fn get_count(&self, count: usize) -> Vec<SignedTransaction> {
        let mut all_txs = vec![];
        let mut leaf_count = count;

        // TODO: Make sure each bucket is average.
        for bucket in self.buckets.iter() {
            let txs = bucket.get_count(leaf_count);
            let txs_len = txs.len();
            all_txs.extend(txs);

            // Avoid overflow
            if leaf_count > txs_len {
                leaf_count -= txs_len
            } else {
                break;
            }
        }
        all_txs
    }

    // TODO: concurrently delete
    pub fn deletes(&self, tx_hashes: &[Hash]) {
        let mut h: HashMap<usize, Vec<Hash>> = HashMap::new();

        for hash in tx_hashes.iter() {
            let index = get_index(hash);
            h.entry(index).or_insert_with(|| vec![]).push(hash.clone());
        }

        h.into_par_iter().for_each(|(index, hashes)| {
            self.buckets[index].deletes(&hashes);
        });
    }

    // TODO: concurrently contains
    pub fn contains_keys(&self, tx_hashes: &[Hash]) -> Vec<Hash> {
        let mut h: HashMap<usize, Vec<Hash>> = HashMap::new();

        for hash in tx_hashes.iter() {
            let index = get_index(hash);
            h.entry(index).or_insert_with(|| vec![]).push(hash.clone());
        }

        let mut all_beingless_keys = vec![];

        for (index, hashes) in h.into_iter() {
            let beingless_keys = self.buckets[index].contains_keys(&hashes);
            all_beingless_keys.extend(beingless_keys);
        }

        all_beingless_keys
    }

    pub fn len(&self) -> usize {
        let mut len = 0;
        for bucket in self.buckets.iter() {
            len += bucket.len();
        }
        len
    }

    // TODO: concurrently clear
    pub fn clear(&self) {
        for bucket in self.buckets.iter() {
            bucket.clear()
        }
    }

    fn get_bucket(&self, hash: &Hash) -> &Bucket {
        &self.buckets[get_index(hash)]
    }
}

fn get_index(hash: &Hash) -> usize {
    (hash.as_bytes()[0] >> 4) as usize
}

struct Bucket {
    store: RwLock<HashMap<Hash, SignedTransaction>>,
}

impl Bucket {
    fn insert(&self, tx: SignedTransaction) {
        self.store.write().insert(tx.hash.clone(), tx);
    }

    fn insert_batch(&self, txs: Vec<SignedTransaction>) {
        let mut store = self.store.write();
        for tx in txs.into_iter() {
            store.insert(tx.hash.clone(), tx);
        }
    }

    fn contains_key(&self, tx_hash: &Hash) -> bool {
        self.store.read().contains_key(tx_hash)
    }

    fn get(&self, tx_hash: &Hash) -> Option<SignedTransaction> {
        self.store.read().get(tx_hash).map(Clone::clone)
    }

    fn get_count(&self, count: usize) -> Vec<SignedTransaction> {
        let store = self.store.read();
        let len = store.len();
        let count = if len < count { len } else { count };

        store
            .par_iter()
            .map(|(_, tx)| tx)
            .collect::<Vec<&SignedTransaction>>()
            .into_par_iter()
            .take(count)
            .map(Clone::clone)
            .collect::<Vec<SignedTransaction>>()
    }

    fn deletes(&self, tx_hashes: &[Hash]) {
        let mut store = self.store.write();
        for hash in tx_hashes {
            store.remove(hash);
        }
    }

    fn contains_keys(&self, tx_hashes: &[Hash]) -> Vec<Hash> {
        let store = self.store.read();
        let mut beingless_keys = vec![];

        for hash in tx_hashes {
            if !store.contains_key(hash) {
                beingless_keys.push(hash.clone())
            }
        }
        beingless_keys
    }

    fn len(&self) -> usize {
        self.store.read().len()
    }

    fn clear(&self) {
        self.store.write().clear();
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use std::sync::Arc;
    use test::Bencher;

    use chashmap::CHashMap;
    use rayon::prelude::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    use super::*;
    use core_types::{Hash, SignedTransaction};

    const GEN_TX_SIZE: usize = 100_000;

    #[test]
    fn test_get_count() {
        let txs = gen_txs(10);
        let cache = TxCache::new(100);
        cache.insert_batch(txs.clone());
        assert_eq!(cache.len(), 10);

        let get_txs = cache.get_count(10000);
        assert_eq!(get_txs.len(), 10);

        let get_txs = cache.get_count(5);
        assert_eq!(get_txs.len(), 5);

        cache.clear();
        let get_txs = cache.get_count(1000);
        assert_eq!(get_txs.len(), 0);
    }

    #[bench]
    fn bench_insert_sharding(b: &mut Bencher) {
        let txs = gen_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = TxCache::new(GEN_TX_SIZE);
            txs.par_iter().for_each(|tx| cache.insert(tx.clone()));
        });
    }

    #[bench]
    fn bench_insert_std(b: &mut Bencher) {
        let txs = gen_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = Arc::new(RwLock::new(HashMap::new()));
            txs.par_iter().for_each(|tx| {
                cache.write().insert(tx.hash.clone(), tx.clone());
            });
        });
    }

    #[bench]
    fn bench_insert_chashmap(b: &mut Bencher) {
        let txs = gen_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = CHashMap::new();
            txs.par_iter().for_each(|tx| {
                cache.insert(tx.hash.clone(), tx.clone());
            });
        });
    }

    fn gen_txs(size: usize) -> Vec<SignedTransaction> {
        let mut txs = Vec::with_capacity(size);
        for _ in 0..size {
            let my_uuid = Uuid::new_v4();
            let mut tx = SignedTransaction::default();
            tx.hash = Hash::digest(my_uuid.as_bytes());
            txs.push(tx)
        }
        txs
    }
}
