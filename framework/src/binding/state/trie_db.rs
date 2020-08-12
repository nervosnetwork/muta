use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use derive_more::{Display, From};
use parking_lot::RwLock;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rocksdb::{Options, WriteBatch, DB};

use common_apm::metrics::storage::{on_storage_get_state, on_storage_put_state};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

// 49999 is the largest prime number within 50000.
const RAND_SEED: u64 = 49999;

pub struct RocksTrieDB {
    light:      bool,
    db:         Arc<DB>,
    cache_size: usize,
    cache:      RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

impl RocksTrieDB {
    pub fn new<P: AsRef<Path>>(
        path: P,
        light: bool,
        max_open_files: i32,
        cache_size: usize,
    ) -> ProtocolResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_max_open_files(max_open_files);

        let db = DB::open(&opts, path).map_err(RocksTrieDBError::from)?;

        // Init HashMap with capacity 2 * cache_size to avoid reallocate memory.
        Ok(RocksTrieDB {
            light,
            db: Arc::new(db),
            cache: RwLock::new(HashMap::with_capacity(cache_size + cache_size)),
            cache_size,
        })
    }

    fn inner_get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RocksTrieDBError> {
        let res = {
            let cache = self.cache.read();
            cache.get(key).cloned()
        };

        if res.is_none() {
            let inst = Instant::now();
            let ret = self.db.get(key).map_err(to_store_err)?;
            on_storage_get_state(inst.elapsed(), 1i64);

            if let Some(val) = ret.clone() {
                let mut cache = self.cache.write();
                cache.insert(key.to_owned(), val);
            }

            return Ok(ret);
        }

        Ok(res)
    }

    #[cfg(test)]
    pub fn insert_batch_without_cache(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) {
        let mut _total_size = 0;
        let mut batch = WriteBatch::default();
        assert_eq!(keys.len(), values.len());

        for (key, val) in keys.iter().zip(values.iter()) {
            _total_size += key.len();
            _total_size += val.len();
            batch.put(key, val);
        }

        self.db.write(batch).unwrap();
    }

    #[cfg(test)]
    pub fn insert_without_cache(&self, key: Vec<u8>, value: Vec<u8>) {
        self.db.put(key, value).unwrap();
    }

    #[cfg(test)]
    pub fn get_without_cache(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.db.get(key).unwrap()
    }

    #[cfg(test)]
    pub fn cache(&self) -> HashMap<Vec<u8>, Vec<u8>> {
        let cache = self.cache.read();
        cache.clone()
    }
}

impl cita_trie::DB for RocksTrieDB {
    type Error = RocksTrieDBError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        self.inner_get(key)
    }

    fn contains(&self, key: &[u8]) -> Result<bool, Self::Error> {
        let res = {
            let cache = self.cache.read();
            cache.contains_key(key)
        };

        if res {
            Ok(true)
        } else {
            if let Some(val) = self.db.get(key).map_err(to_store_err)? {
                let mut cache = self.cache.write();
                cache.insert(key.to_owned(), val);
                return Ok(true);
            }
            Ok(false)
        }
    }

    fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Self::Error> {
        let inst = Instant::now();
        let size = key.len() + value.len();

        {
            let mut cache = self.cache.write();
            cache.insert(key.clone(), value.clone());
        }

        self.db
            .put(Bytes::from(key), Bytes::from(value))
            .map_err(to_store_err)?;

        on_storage_put_state(inst.elapsed(), size as i64);
        Ok(())
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        if keys.len() != values.len() {
            return Err(RocksTrieDBError::BatchLengthMismatch);
        }

        let mut total_size = 0;
        let mut batch = WriteBatch::default();

        {
            let mut cache = self.cache.write();
            for (key, val) in keys.iter().zip(values.iter()) {
                total_size += key.len();
                total_size += val.len();
                batch.put(key, val);
                cache.insert(key.clone(), val.clone());
            }
        }

        let inst = Instant::now();
        self.db.write(batch).map_err(to_store_err)?;
        on_storage_put_state(inst.elapsed(), total_size as i64);
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        if self.light {
            {
                let mut cache = self.cache.write();
                cache.remove(key);
            }
            self.db.delete(key).map_err(to_store_err)?;
        }
        Ok(())
    }

    fn remove_batch(&self, keys: &[Vec<u8>]) -> Result<(), Self::Error> {
        if self.light {
            let mut batch = WriteBatch::default();
            {
                let mut cache = self.cache.write();
                for key in keys {
                    batch.delete(key);
                    cache.remove(key);
                }
            }

            self.db.write(batch).map_err(to_store_err)?;
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        let mut cache = self.cache.write();
        let len = cache.len();

        if len <= self.cache_size {
            return Ok(());
        }

        let keys = cache.keys().collect::<Vec<_>>();
        let remove_list = rand_remove_list(keys, len - self.cache_size);

        for item in remove_list.iter() {
            cache.remove(item);
        }
        Ok(())
    }
}

fn rand_remove_list<T: Clone>(keys: Vec<&T>, num: usize) -> Vec<T> {
    let mut len = keys.len() - 1;
    let mut idx_list = (0..len).collect::<Vec<_>>();
    let mut rng = SmallRng::seed_from_u64(RAND_SEED);
    let mut ret = Vec::new();

    for _ in 0..num {
        let tmp = rng.gen_range(0, len);
        let idx = idx_list.remove(tmp);
        ret.push(keys[idx].to_owned());
        len -= 1;
    }
    ret
}

#[derive(Debug, Display, From)]
pub enum RocksTrieDBError {
    #[display(fmt = "store error")]
    Store,

    #[display(fmt = "rocksdb {}", _0)]
    RocksDB(rocksdb::Error),

    #[display(fmt = "parameters do not match")]
    InsertParameter,

    #[display(fmt = "batch length dont match")]
    BatchLengthMismatch,
}

impl std::error::Error for RocksTrieDBError {}

impl From<RocksTrieDBError> for ProtocolError {
    fn from(err: RocksTrieDBError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}

fn to_store_err(e: rocksdb::Error) -> RocksTrieDBError {
    log::error!("[framework] trie db {:?}", e);
    RocksTrieDBError::Store
}

#[cfg(test)]
mod tests {
    extern crate test;
    use test::Bencher;

    use super::*;

    #[bench]
    fn bench_rand(b: &mut Bencher) {
        b.iter(|| {
            let mut rng = SmallRng::seed_from_u64(RAND_SEED);
            for _ in 0..10000 {
                rng.gen_range(10, 1000000);
            }
        })
    }

    #[test]
    fn test_rand_remove() {
        let list = (0..10).collect::<Vec<_>>();
        let keys = list.iter().collect::<Vec<_>>();
        let to_removed_num = (1..10).collect::<Vec<_>>();

        for num in to_removed_num.into_iter() {
            let res = rand_remove_list(keys.clone(), num);
            assert_eq!(res.len(), num);
        }
    }
}
