use std::collections::HashMap;

use parking_lot::RwLock;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};

use protocol::types::Hash;

/// The "Map" is a concurrent HashMap that uses 16 buckets to
/// decentralize store transactions.
/// Why use 16 buckets? We take 0 bytes of each "tx_hash" and shift it 4 bits to
/// the left to get a number in the range 0~15, which corresponds to 16 buckets.
pub struct Map<V> {
    buckets: Vec<Bucket<V>>,
}

impl<V> Map<V>
where
    V: Send + Sync + Clone,
{
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

    pub fn insert(&self, tx_hash: Hash, value: V) -> Option<V> {
        let bucket = self.get_bucket(&tx_hash);
        bucket.insert(tx_hash, value)
    }

    pub fn contains_key(&self, tx_hash: &Hash) -> bool {
        let bucket = self.get_bucket(tx_hash);
        bucket.contains_key(tx_hash)
    }

    pub fn get(&self, tx_hash: &Hash) -> Option<V> {
        let bucket = self.get_bucket(tx_hash);
        bucket.get(tx_hash)
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

    pub fn remove(&self, tx_hash: &Hash) {
        let index = get_index(tx_hash);
        self.buckets[index].remove(tx_hash);
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

    fn get_bucket(&self, hash: &Hash) -> &Bucket<V> {
        &self.buckets[get_index(hash)]
    }
}

fn get_index(hash: &Hash) -> usize {
    (hash.as_bytes()[0] >> 4) as usize
}

struct Bucket<V> {
    store: RwLock<HashMap<Hash, V>>,
}

impl<V> Bucket<V>
where
    V: Send + Sync + Clone,
{
    fn insert(&self, hash: Hash, value: V) -> Option<V> {
        let mut lock_data = self.store.write();
        if lock_data.contains_key(&hash) {
            Some(value)
        } else {
            lock_data.insert(hash, value)
        }
    }

    fn contains_key(&self, tx_hash: &Hash) -> bool {
        self.store.read().contains_key(tx_hash)
    }

    fn get(&self, tx_hash: &Hash) -> Option<V> {
        self.store.read().get(tx_hash).map(Clone::clone)
    }

    fn deletes(&self, tx_hashes: &[Hash]) {
        let mut store = self.store.write();
        for hash in tx_hashes {
            store.remove(hash);
        }
    }

    fn remove(&self, tx_hash: &Hash) {
        let mut store = self.store.write();
        store.remove(tx_hash);
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

    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    use bytes::Bytes;
    use chashmap::CHashMap;
    use rand::random;
    use rayon::prelude::*;
    use test::Bencher;

    use protocol::types::Hash;

    use crate::map::Map;

    const GEN_TX_SIZE: usize = 1000;

    #[bench]
    fn bench_map_insert(b: &mut Bencher) {
        let txs = mock_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = Map::new(GEN_TX_SIZE);
            txs.par_iter().for_each(|(hash, tx)| {
                cache.insert(hash.clone(), tx.clone());
            });
        });
    }

    #[bench]
    fn bench_std_map_insert(b: &mut Bencher) {
        let txs = mock_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = Arc::new(RwLock::new(HashMap::new()));
            txs.par_iter().for_each(|(hash, tx)| {
                cache.write().unwrap().insert(hash, tx);
            });
        });
    }

    #[bench]
    fn bench_chashmap_insert(b: &mut Bencher) {
        let txs = mock_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = CHashMap::new();
            txs.par_iter().for_each(|(hash, tx)| {
                cache.insert(hash, tx);
            });
        });
    }

    fn mock_txs(size: usize) -> Vec<(Hash, Hash)> {
        let mut txs = Vec::with_capacity(size);
        for _ in 0..size {
            let tx: Vec<u8> = (0..10).map(|_| random::<u8>()).collect();
            let tx = Hash::digest(Bytes::from(tx));
            txs.push((tx.clone(), tx));
        }
        txs
    }
}
