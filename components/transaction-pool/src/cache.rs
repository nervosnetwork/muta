use hashbrown::HashMap;
use parking_lot::RwLock;

use core_types::{Hash, SignedTransaction};

/// The "TxCache" is a transaction pool cache that uses 16 buckets to
/// decentralize store transactions.
/// Why use 16 buckets? We take 0 bytes of each "tx_hash" and shift it 4 bits to
/// the left to get a number in the range 0~15, which corresponds to 16 buckets.
pub struct TxCache {
    buckets: Vec<Bucket>,
}

impl TxCache {
    pub fn new() -> Self {
        let mut buckets = Vec::with_capacity(16);
        for _ in 0..16 {
            buckets.push(Bucket {
                store: RwLock::new(HashMap::new()),
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
        // TODO: https://github.com/cryptape/muta/pull/217/files/697ca72df9192937abf1fb2c253d06ca213d85a4#r283665498
        let avg_len = count / self.buckets.len();
        let mut all_txs = vec![];

        // get the same length of transactions from each bucket.
        for bucket in self.buckets.iter() {
            let txs = bucket.get_count(avg_len);
            all_txs.extend(txs);
        }
        if all_txs.len() == count {
            return all_txs;
        }

        // If we don't get enough transactions, start the loop from the first bukcet
        // until the conditions are met.
        let mut left_len = count - all_txs.len();
        for bucket in self.buckets.iter() {
            let txs = bucket.get_count(left_len);
            let txs_len = txs.len();
            all_txs.extend(txs);

            left_len -= txs_len;
            if left_len == 0 {
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

        for (index, hashes) in h.into_iter() {
            self.buckets[index].deletes(&hashes);
        }
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

        let mut txs = Vec::with_capacity(count);
        for (_, tx) in store.iter() {
            txs.push(tx.clone());
        }
        txs
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
    use hashbrown::HashMap;
    use rayon::prelude::*;
    use uuid::Uuid;

    use super::*;
    use core_types::{Hash, SignedTransaction};

    const GEN_TX_SIZE: usize = 100_000;

    #[bench]
    fn bench_insert_sharding(b: &mut Bencher) {
        let txs = gen_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = TxCache::new();
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
