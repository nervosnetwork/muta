use std::collections::HashMap;
use std::sync::Arc;

use futures::future::try_join_all;
use tokio::sync::RwLock;

use protocol::types::Hash;

/// The "Map" is a concurrent HashMap that uses 16 buckets to
/// decentralize store transactions.
/// Why use 16 buckets? We take 0 bytes of each "tx_hash" and shift it 4 bits to
/// the left to get a number in the range 0~15, which corresponds to 16 buckets.
pub struct Map<V> {
    buckets: Vec<Arc<Bucket<V>>>,
}

impl<V> Map<V>
where
    V: Send + Sync + Clone + 'static,
{
    pub fn new(cache_size: usize) -> Self {
        let mut buckets = Vec::with_capacity(16);
        for _ in 0..16 {
            buckets.push(Arc::new(Bucket {
                // Allocate enough space to avoid triggering resize.
                store: RwLock::new(HashMap::with_capacity(cache_size)),
            }));
        }
        Self { buckets }
    }

    pub async fn insert(&self, hash: Hash, value: V) -> Option<V> {
        let bucket = self.get_bucket(&hash);
        bucket.insert(hash, value).await
    }

    pub async fn contains_key(&self, hash: &Hash) -> bool {
        let bucket = self.get_bucket(hash);
        bucket.contains_key(hash).await
    }

    pub async fn get(&self, hash: &Hash) -> Option<V> {
        let bucket = self.get_bucket(hash);
        bucket.get(hash).await
    }

    pub async fn remove(&self, hash: &Hash) {
        let bucket = self.get_bucket(hash);
        bucket.remove(hash).await
    }

    pub async fn remove_batch(&self, hashes: &[Hash]) {
        let mut h: HashMap<usize, Vec<Hash>> = HashMap::new();

        for hash in hashes.iter() {
            let index = get_index(hash);
            h.entry(index).or_insert_with(Vec::new).push(hash.clone());
        }

        let futs = h
            .into_iter()
            .map(|(index, hashes)| {
                let bucket = Arc::clone(&self.buckets[index]);
                tokio::spawn(async move { bucket.remove_batch(hashes).await })
            })
            .collect::<Vec<_>>();
        try_join_all(futs)
            .await
            .expect("[mempool]: the runtime panics.");
    }

    pub async fn len(&self) -> usize {
        let mut len = 0;
        for bucket in self.buckets.iter() {
            len += bucket.len().await;
        }
        len
    }

    pub async fn clear(&self) {
        let futs = self
            .buckets
            .iter()
            .map(|bucket| {
                let bucket = Arc::clone(bucket);
                tokio::spawn(async move { bucket.clear().await })
            })
            .collect::<Vec<_>>();

        try_join_all(futs)
            .await
            .expect("[mempool]: the runtime panics.");
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
    /// Before inserting a transaction into the bucket, you must check whether
    /// the transaction is in the bucket first. Never use the insert function to
    /// check this.
    async fn insert(&self, hash: Hash, value: V) -> Option<V> {
        let mut lock_data = self.store.write().await;
        if lock_data.contains_key(&hash) {
            Some(value)
        } else {
            lock_data.insert(hash, value)
        }
    }

    async fn contains_key(&self, hash: &Hash) -> bool {
        self.store.read().await.contains_key(hash)
    }

    async fn get(&self, hash: &Hash) -> Option<V> {
        self.store.read().await.get(hash).map(Clone::clone)
    }

    async fn remove(&self, hash: &Hash) {
        let mut store = self.store.write().await;
        store.remove(hash);
    }

    async fn remove_batch(&self, hashes: Vec<Hash>) {
        let mut store = self.store.write().await;
        for hash in hashes {
            store.remove(&hash);
        }
    }

    async fn len(&self) -> usize {
        self.store.read().await.len()
    }

    async fn clear(&self) {
        self.store.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    use chashmap::CHashMap;
    use rand::random;
    use test::Bencher;

    use protocol::{types::Hash, Bytes};

    use crate::map::Map;

    const GEN_TX_SIZE: usize = 1000;

    #[bench]
    fn bench_map_insert(b: &mut Bencher) {
        let mut runtime = tokio::runtime::Runtime::new().unwrap();

        let txs = mock_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = Map::new(GEN_TX_SIZE);
            txs.iter().for_each(|(hash, tx)| {
                runtime.block_on(cache.insert(hash.clone(), tx.clone()));
            });
        });
    }

    #[bench]
    fn bench_std_map_insert(b: &mut Bencher) {
        let txs = mock_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = Arc::new(RwLock::new(HashMap::new()));
            txs.iter().for_each(|(hash, tx)| {
                cache.write().unwrap().insert(hash, tx);
            });
        });
    }

    #[bench]
    fn bench_chashmap_insert(b: &mut Bencher) {
        let txs = mock_txs(GEN_TX_SIZE);

        b.iter(move || {
            let cache = CHashMap::new();
            txs.iter().for_each(|(hash, tx)| {
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
