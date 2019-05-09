use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use core_types::{Hash, SignedTransaction};

#[derive(Debug)]
pub struct Cache {
    store: Arc<RwLock<HashMap<Hash, SignedTransaction>>>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn insert(&self, tx: SignedTransaction) {
        self.store.write().insert(tx.hash.clone(), tx);
    }

    pub fn insert_batch(&self, txs: Vec<SignedTransaction>) {
        let mut store = self.store.write();
        for tx in txs.into_iter() {
            store.insert(tx.hash.clone(), tx);
        }
    }

    pub fn contains_key(&self, tx_hash: &Hash) -> bool {
        self.store.read().contains_key(tx_hash)
    }

    pub fn get(&self, tx_hash: &Hash) -> Option<SignedTransaction> {
        self.store.read().get(tx_hash).map(Clone::clone)
    }

    pub fn get_count(&self, count: usize) -> Vec<SignedTransaction> {
        let store = self.store.read();
        let len = store.len();
        let count = if len < count { len } else { count };

        let mut txs = Vec::with_capacity(count);
        for (_, tx) in store.iter() {
            txs.push(tx.clone());
        }
        txs
    }

    pub fn deletes(&self, tx_hashes: &[Hash]) {
        let mut store = self.store.write();
        for hash in tx_hashes {
            store.remove(hash);
        }
    }

    pub fn contains_keys(&self, tx_hashes: &[Hash]) -> Vec<Hash> {
        let store = self.store.read();
        let mut not_contains = vec![];

        for hash in tx_hashes {
            if !store.contains_key(hash) {
                not_contains.push(hash.clone())
            }
        }
        not_contains
    }

    pub fn len(&self) -> usize {
        self.store.read().len()
    }

    pub fn clear(&self) {
        self.store.write().clear();
    }
}

impl Clone for Cache {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
        }
    }
}
