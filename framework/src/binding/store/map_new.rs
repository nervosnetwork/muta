use std::cell::RefCell;
use std::iter::Iterator;
use std::marker::PhantomData;
use std::rc::Rc;

use bytes::Bytes;
use rayon::prelude::*;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{ServiceState, StoreMap};
use protocol::types::Hash;
use protocol::{ProtocolError, ProtocolResult};

use crate::binding::store::{get_bucket_index, Bucket, FixedBuckets, StoreError};

pub struct NewStoreMap<S: ServiceState, K: FixedCodec + PartialEq, V: FixedCodec> {
    state:    Rc<RefCell<S>>,
    var_name: String,
    keys:     FixedBuckets<K>,
    phantom:  PhantomData<V>,
}

impl<S, K, V> NewStoreMap<S, K, V>
where
    S: 'static + ServiceState,
    K: 'static + Send + FixedCodec + PartialEq,
    V: 'static + FixedCodec,
{
    pub fn new(state: Rc<RefCell<S>>, name: &str) -> Self {
        let var_name = name.to_string();
        let prefix = name.to_string() + "map_";

        let opt_bytes = (0..16)
            .map(|idx| {
                let hash = Hash::digest(Bytes::from(prefix.clone() + &idx.to_string()));
                state.borrow().get(&hash).unwrap()
            })
            .collect::<Vec<_>>();

        let buckets = opt_bytes
            .into_par_iter()
            .map(|bytes| {
                if let Some(bs) = bytes {
                    <_>::decode_fixed(bs).expect("")
                } else {
                    Bucket::new()
                }
            })
            .collect::<Vec<_>>();

        Self {
            state,
            var_name,
            keys: FixedBuckets::new(buckets),

            phantom: PhantomData,
        }
    }

    fn inner_insert(&mut self, key: K, value: V) -> ProtocolResult<()> {
        let key_bytes = key.encode_fixed()?;
        let mk = self.get_map_key(&key_bytes);
        let bkt_idx = get_bucket_index(&key_bytes);

        if !self.inner_contains(&key, &key_bytes) {
            self.keys.insert(bkt_idx, key);

            self.state.borrow_mut().insert(
                self.get_bucket_name(bkt_idx),
                self.keys.get_bucket(bkt_idx).encode_fixed()?,
            )?;
        }
        self.state.borrow_mut().insert(mk, value)
    }

    fn inner_get(&self, key: &K) -> ProtocolResult<Option<V>> {
        let key_bytes = key.encode_fixed()?;
        if self.inner_contains(key, &key_bytes) {
            self.state
                .borrow()
                .get(&self.get_map_key(&key_bytes))?
                .map_or_else(
                    || {
                        Ok(Some(<_>::decode_fixed(Bytes::new()).map_err(|_| {
                            ProtocolError::from(StoreError::DecodeError)
                        })?))
                    },
                    |v| Ok(Some(v)),
                )
        } else {
            Ok(None)
        }
    }

    fn inner_remove(&mut self, key: &K) -> ProtocolResult<Option<V>> {
        let key_bytes = key.encode_fixed()?;
        if self.inner_contains(key, &key_bytes) {
            let value = self.inner_get(key)?.expect("value should be existed");
            let bkt_idx = get_bucket_index(&key_bytes);
            let bkt_name = self.get_bucket_name(bkt_idx);

            let _ = self.keys.remove_item(key, &key_bytes)?;
            self.state
                .borrow_mut()
                .insert(bkt_name, self.keys.get_bucket(bkt_idx).encode_fixed()?)?;
            self.state
                .borrow_mut()
                .insert(self.get_map_key(&key_bytes), Bytes::new())?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    fn inner_contains(&self, key: &K, key_bytes: &Bytes) -> bool {
        self.keys.contains(key, key_bytes)
    }

    fn get_map_key(&self, key_bytes: &Bytes) -> Hash {
        let mut name_bytes = self.var_name.as_bytes().to_vec();
        name_bytes.extend_from_slice(key_bytes);
        Hash::digest(Bytes::from(name_bytes))
    }

    fn get_bucket_name(&self, index: usize) -> Hash {
        Hash::digest(Bytes::from(
            self.var_name.clone() + "map_" + &index.to_string(),
        ))
    }
}

impl<S, K, V> StoreMap<K, V> for NewStoreMap<S, K, V>
where
    S: 'static + ServiceState,
    K: 'static + Send + FixedCodec + PartialEq,
    V: 'static + FixedCodec,
{
    fn get(&self, key: &K) -> Option<V> {
        self.inner_get(key)
            .unwrap_or_else(|e| panic!("StoreMap get failed: {}", e))
    }

    fn insert(&mut self, key: K, value: V) {
        self.inner_insert(key, value)
            .unwrap_or_else(|e| panic!("StoreMap insert failed: {}", e));
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.inner_remove(key)
            .unwrap_or_else(|e| panic!("StoreMap remove failed: {}", e))
    }

    fn contains(&self, key: &K) -> bool {
        if let Ok(bytes) = key.encode_fixed() {
            self.inner_contains(key, &bytes)
        } else {
            false
        }
    }

    fn len(&self) -> u32 {
        self.keys.len()
    }

    fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (&K, V)> + 'a> {
        Box::new(NewMapIter::<S, K, V>::new(0, self))
    }
}

pub struct NewMapIter<
    'a,
    S: 'static + ServiceState,
    K: 'static + FixedCodec + PartialEq,
    V: 'static + FixedCodec,
> {
    idx: u32,
    map: &'a NewStoreMap<S, K, V>,
}

impl<'a, S, K, V> NewMapIter<'a, S, K, V>
where
    S: 'static + ServiceState,
    K: 'static + FixedCodec + PartialEq,
    V: 'static + FixedCodec,
{
    pub fn new(idx: u32, map: &'a NewStoreMap<S, K, V>) -> Self {
        Self { idx, map }
    }
}

impl<'a, S, K, V> Iterator for NewMapIter<'a, S, K, V>
where
    S: 'static + ServiceState,
    K: 'static + Send + FixedCodec + PartialEq,
    V: 'static + FixedCodec,
{
    type Item = (&'a K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.idx;
        if idx >= self.map.keys.len() {
            return None;
        }

        for i in 0..16 {
            let (left, right) = self.map.keys.get_abs_index_interval(i);
            if left <= idx && idx < right {
                let index = idx - left;
                let key = self.map.keys.keys_bucket[i]
                    .0
                    .get(index as usize)
                    .expect("get key should not fail");

                self.idx += 1;
                return Some((key, self.map.get(key).expect("get value should not fail")));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use std::path::PathBuf;
    use std::sync::Arc;

    use muta_codec_derive::RlpFixedCodec;
    use rand::random;
    use serde::{Deserialize, Serialize};
    use test::Bencher;

    use protocol::fixed_codec::FixedCodecError;
    use protocol::types::Address;

    use crate::binding::state::{GeneralServiceState, MPTTrie, RocksTrieDB};
    use crate::binding::store::DefaultStoreMap;

    use super::*;

    #[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
    pub struct Asset {
        pub id:     Hash,
        pub name:   String,
        pub symbol: String,
        pub supply: u64,
        pub issuer: Address,
    }

    impl Asset {
        fn new() -> Self {
            Asset {
                id:     Hash::digest(Bytes::from(random::<u64>().to_string().into_bytes())),
                name:   "muta_token".to_string(),
                symbol: "muta_token".to_string(),
                supply: random::<u64>(),
                issuer: Address::from_bytes(Bytes::from(
                    (0..20).map(|_| random::<u8>()).collect::<Vec<_>>(),
                ))
                .unwrap(),
            }
        }
    }

    #[bench]
    fn bench_create_default_map(b: &mut Bencher) {
        let path = PathBuf::from("./data/default/create");

        let state = Rc::new(RefCell::new(GeneralServiceState::new(MPTTrie::new(
            Arc::new(RocksTrieDB::new(path, false, 1024).unwrap()),
        ))));
        let state_clone = Rc::clone(&state);
        let mut map = DefaultStoreMap::<_, Hash, Asset>::new(state, "asset");

        for _i in 0..1000 {
            let id = rand::random::<u32>().to_string();
            let id_hash = Hash::digest(Bytes::from(id.into_bytes()));
            map.insert(id_hash, Asset::new());
        }

        b.iter(move || {
            let _ = DefaultStoreMap::<_, Hash, Asset>::new(Rc::clone(&state_clone), "asset");
        })
    }

    #[bench]
    fn bench_create_new_map(b: &mut Bencher) {
        let path = PathBuf::from("./data/new/create");

        let state = Rc::new(RefCell::new(GeneralServiceState::new(MPTTrie::new(
            Arc::new(RocksTrieDB::new(path, false, 1024).unwrap()),
        ))));
        let state_clone = Rc::clone(&state);
        let mut map = NewStoreMap::<_, Hash, Asset>::new(state, "asset");

        for _i in 0..100000 {
            let id = rand::random::<u32>().to_string();
            let id_hash = Hash::digest(Bytes::from(id.into_bytes()));
            map.insert(id_hash, Asset::new());
        }

        b.iter(move || {
            let _ = DefaultStoreMap::<_, Hash, Asset>::new(Rc::clone(&state_clone), "asset");
        })
    }

    #[bench]
    fn bench_insert_default_map(b: &mut Bencher) {
        let path = PathBuf::from("./data/default/insert");

        let state = Rc::new(RefCell::new(GeneralServiceState::new(MPTTrie::new(
            Arc::new(RocksTrieDB::new(path, false, 1024).unwrap()),
        ))));
        let mut map = DefaultStoreMap::<_, Hash, Asset>::new(state, "asset");

        for _i in 0..1000 {
            let id = rand::random::<u32>().to_string();
            let id_hash = Hash::digest(Bytes::from(id.into_bytes()));
            map.insert(id_hash, Asset::new());
        }

        let hash = Hash::digest(Bytes::from(rand::random::<u32>().to_string().into_bytes()));
        let asset = Asset::new();

        b.iter(move || {
            map.insert(hash.clone(), asset.clone());
        })
    }

    #[bench]
    fn bench_insert_new_map(b: &mut Bencher) {
        let path = PathBuf::from("./data/new/insert");

        let state = Rc::new(RefCell::new(GeneralServiceState::new(MPTTrie::new(
            Arc::new(RocksTrieDB::new(path, false, 1024).unwrap()),
        ))));
        let mut map = NewStoreMap::<_, Hash, Asset>::new(state, "asset");

        for _i in 0..1000 {
            let id = rand::random::<u32>().to_string();
            let id_hash = Hash::digest(Bytes::from(id.into_bytes()));
            map.insert(id_hash, Asset::new());
        }

        let hash = Hash::digest(Bytes::from(rand::random::<u32>().to_string().into_bytes()));
        let asset = Asset::new();

        b.iter(move || {
            map.insert(hash.clone(), asset.clone());
        })
    }

    #[bench]
    fn bench_iter_default_map(b: &mut Bencher) {
        let path = PathBuf::from("./data/default/iter");

        let state = Rc::new(RefCell::new(GeneralServiceState::new(MPTTrie::new(
            Arc::new(RocksTrieDB::new(path, false, 1024).unwrap()),
        ))));
        let mut map = DefaultStoreMap::<_, Hash, Asset>::new(state, "asset");

        for _i in 0..1000 {
            let id = rand::random::<u32>().to_string();
            let id_hash = Hash::digest(Bytes::from(id.into_bytes()));
            map.insert(id_hash, Asset::new());
        }

        b.iter(move || for _ in map.iter() {})
    }

    #[bench]
    fn bench_iter_new_map(b: &mut Bencher) {
        let path = PathBuf::from("./data/new/iter");

        let state = Rc::new(RefCell::new(GeneralServiceState::new(MPTTrie::new(
            Arc::new(RocksTrieDB::new(path, false, 1024).unwrap()),
        ))));
        let mut map = NewStoreMap::<_, Hash, Asset>::new(state, "asset");

        for _i in 0..1000 {
            let id = rand::random::<u32>().to_string();
            let id_hash = Hash::digest(Bytes::from(id.into_bytes()));
            map.inner_insert(id_hash, Asset::new()).unwrap();
        }

        b.iter(move || for _ in map.iter() {})
    }
}
