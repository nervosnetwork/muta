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

pub struct DefaultStoreMap<S: ServiceState, K: FixedCodec + PartialEq, V: FixedCodec> {
    state:    Rc<RefCell<S>>,
    var_name: String,
    keys:     RefCell<FixedBuckets<K>>,
    len_key:  Bytes,
    len:      u32,
    phantom:  PhantomData<V>,
}

impl<S, K, V> DefaultStoreMap<S, K, V>
where
    S: 'static + ServiceState,
    K: 'static + Send + FixedCodec + PartialEq,
    V: 'static + FixedCodec,
{
    pub fn new(state: Rc<RefCell<S>>, name: &str) -> Self {
        let len_key = Bytes::from(name.to_string() + "_map_len");
        let len = state
            .borrow()
            .get(&len_key)
            .expect("Get len failed")
            .unwrap_or(0u32);

        DefaultStoreMap {
            state,
            len_key,
            len,
            var_name: name.to_string(),
            keys: RefCell::new(FixedBuckets::new()),
            phantom: PhantomData,
        }
    }

    fn inner_insert(&mut self, key: K, value: V) -> ProtocolResult<()> {
        let key_bytes = key.encode_fixed()?;
        let mk = self.get_map_key(&key_bytes);
        let bkt_idx = get_bucket_index(&key_bytes);

        if !self.inner_contains(bkt_idx, &key)? {
            self.keys.borrow_mut().insert(bkt_idx, key);

            self.state.borrow_mut().insert(
                self.get_bucket_name(bkt_idx),
                self.keys.borrow().get_bucket(bkt_idx).encode_fixed()?,
            )?;
            self.len_add_one()?;
        }
        self.state.borrow_mut().insert(mk, value)
    }

    fn inner_get(&self, key: &K) -> ProtocolResult<Option<V>> {
        let key_bytes = key.encode_fixed()?;
        let bkt_idx = get_bucket_index(&key_bytes);

        if self.inner_contains(bkt_idx, &key)? {
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
        let bkt_idx = get_bucket_index(&key_bytes);

        if self.inner_contains(bkt_idx, &key)? {
            let value = self.inner_get(key)?.expect("value should be existed");
            let bkt_idx = get_bucket_index(&key_bytes);
            let bkt_name = self.get_bucket_name(bkt_idx);

            let _ = self.keys.borrow_mut().remove_item(bkt_idx, key)?;
            self.state.borrow_mut().insert(
                bkt_name,
                self.keys.borrow().get_bucket(bkt_idx).encode_fixed()?,
            )?;
            self.state
                .borrow_mut()
                .insert(self.get_map_key(&key_bytes), Bytes::new())?;
            self.len_sub_one()?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    #[inline(always)]
    fn inner_contains(&self, bkt_idx: usize, key: &K) -> ProtocolResult<bool> {
        if self.keys.borrow().is_bucket_recovered(bkt_idx) {
            return Ok(self.keys.borrow().contains(bkt_idx, key));
        }

        let bkt = if let Some(bytes) = self.state.borrow().get(&self.get_bucket_name(bkt_idx))? {
            <_>::decode_fixed(bytes)?
        } else {
            Bucket::new()
        };

        let ret = bkt.contains(key);
        self.keys.borrow_mut().recover_bucket(bkt_idx, bkt);
        Ok(ret)
    }

    fn get_map_key(&self, key_bytes: &Bytes) -> Bytes {
        let mut name_bytes = self.var_name.as_bytes().to_vec();
        name_bytes.extend_from_slice(key_bytes);

        if key_bytes.len() > 32 {
            Hash::digest(Bytes::from(name_bytes)).as_bytes()
        } else {
            Bytes::from(name_bytes)
        }
    }

    fn get_bucket_name(&self, index: usize) -> Bytes {
        let mut bytes = (self.var_name.clone() + "_bucket_").as_bytes().to_vec();
        bytes.extend_from_slice(&index.to_le_bytes());
        Bytes::from(bytes)
    }

    fn len_add_one(&mut self) -> ProtocolResult<()> {
        self.len += 1;
        self.state
            .borrow_mut()
            .insert(self.len_key.clone(), self.len.encode_fixed()?)
    }

    fn len_sub_one(&mut self) -> ProtocolResult<()> {
        self.len -= 1;
        self.state
            .borrow_mut()
            .insert(self.len_key.clone(), self.len.encode_fixed()?)
    }

    fn recover_all_buckets(&self) {
        let idxs = self
            .keys
            .borrow()
            .is_recovered
            .iter()
            .enumerate()
            .filter_map(|(i, &res)| if !res { Some(i) } else { None })
            .collect::<Vec<_>>();

        let opt_bytes = idxs
            .iter()
            .map(|idx| {
                let name = self.get_bucket_name(*idx);
                self.state.borrow().get(&name).unwrap()
            })
            .collect::<Vec<_>>();

        let buckets = opt_bytes
            .into_par_iter()
            .map(|bytes| {
                if let Some(bs) = bytes {
                    <_>::decode_fixed(bs).expect("Decode bucket failed")
                } else {
                    Bucket::new()
                }
            })
            .collect::<Vec<_>>();

        for (idx, bkt) in idxs.into_iter().zip(buckets.into_iter()) {
            self.keys.borrow_mut().recover_bucket(idx, bkt);
        }
    }

    #[cfg(test)]
    fn get_buckets(self) -> FixedBuckets<K> {
        self.keys.into_inner()
    }
}

impl<S, K, V> StoreMap<K, V> for DefaultStoreMap<S, K, V>
where
    S: 'static + ServiceState,
    K: 'static + Send + FixedCodec + Clone + PartialEq,
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
            self.inner_contains(get_bucket_index(&bytes), &key)
                .unwrap_or(false)
        } else {
            false
        }
    }

    fn len(&self) -> u32 {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (K, V)> + 'a> {
        self.recover_all_buckets();
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
    map: &'a DefaultStoreMap<S, K, V>,
}

impl<'a, S, K, V> NewMapIter<'a, S, K, V>
where
    S: 'static + ServiceState,
    K: 'static + FixedCodec + PartialEq,
    V: 'static + FixedCodec,
{
    pub fn new(idx: u32, map: &'a DefaultStoreMap<S, K, V>) -> Self {
        Self { idx, map }
    }
}

impl<'a, S, K, V> Iterator for NewMapIter<'a, S, K, V>
where
    S: 'static + ServiceState,
    K: 'static + Send + FixedCodec + Clone + PartialEq,
    V: 'static + FixedCodec,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.idx;
        if idx >= self.map.len {
            return None;
        }

        for i in 0..16 {
            let (left, right) = self.map.keys.borrow().get_abs_index_interval(i);
            if left <= idx && idx < right {
                let index = idx - left;
                let key = self.map.keys.borrow().keys_bucket[i]
                    .0
                    .get(index as usize)
                    .cloned()
                    .expect("get key should not fail");

                self.idx += 1;
                return Some((
                    key.clone(),
                    self.map.get(&key).expect("get value should not fail"),
                ));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cita_trie::MemoryDB;
    use rand::random;

    use crate::binding::state::{GeneralServiceState, MPTTrie};
    use crate::binding::store::map::DefaultStoreMap;

    use super::*;

    fn gen_bytes() -> Bytes {
        Bytes::from((0..16).map(|_| random::<u8>()).collect::<Vec<_>>())
    }

    #[test]
    fn test_map_and_bucket() {
        let state = Rc::new(RefCell::new(GeneralServiceState::new(MPTTrie::new(
            Arc::new(MemoryDB::new(false)),
        ))));
        let mut map = DefaultStoreMap::<_, Bytes, Bytes>::new(Rc::clone(&state), "test");
        let key_1 = gen_bytes();
        let val_1 = gen_bytes();
        let key_2 = gen_bytes();
        let val_2 = gen_bytes();
        let key_idx_1 = get_bucket_index(&key_1.encode_fixed().unwrap());
        let key_idx_2 = get_bucket_index(&key_2.encode_fixed().unwrap());

        map.insert(key_1, val_1);
        map.insert(key_2, val_2);

        assert_eq!(map.len(), 2);

        let fbkt = map.get_buckets();
        assert!(fbkt.is_recovered[key_idx_1]);
        assert!(fbkt.is_recovered[key_idx_2]);
        assert_eq!(fbkt.len(), 2);

        let max = key_idx_1.max(key_idx_2);
        let min = key_idx_1.min(key_idx_2);
        let res = (0..17)
            .map(|i| {
                if i > max {
                    2u32
                } else if i > min {
                    1u32
                } else {
                    0u32
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(fbkt.bucket_lens, res);
    }
}
