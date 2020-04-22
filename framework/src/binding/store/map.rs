use std::cell::RefCell;
use std::iter::Iterator;
use std::marker::PhantomData;
use std::rc::Rc;

use bytes::Bytes;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{ServiceState, StoreMap};
use protocol::types::Hash;
use protocol::{ProtocolError, ProtocolResult};

use crate::binding::store::{FixedKeys, StoreError};

pub struct DefaultStoreMap<S: ServiceState, K: FixedCodec + PartialEq, V: FixedCodec> {
    state:    Rc<RefCell<S>>,
    var_name: Hash,
    keys:     FixedKeys<K>,
    phantom:  PhantomData<V>,
}

impl<S: 'static + ServiceState, K: 'static + FixedCodec + PartialEq, V: 'static + FixedCodec>
    DefaultStoreMap<S, K, V>
{
    pub fn new(state: Rc<RefCell<S>>, name: &str) -> Self {
        let var_name = Hash::digest(Bytes::from(name.to_owned() + "map"));

        let opt_bs: Option<Bytes> = state
            .borrow()
            .get(&var_name)
            .expect("get map should not fail");

        let keys = if let Some(bs) = opt_bs {
            <_>::decode_fixed(bs).expect("decode keys should not fail")
        } else {
            FixedKeys { inner: Vec::new() }
        };

        Self {
            state,
            var_name,
            keys,
            phantom: PhantomData,
        }
    }

    fn get_map_key(&self, key: &K) -> ProtocolResult<Hash> {
        let mut name_bytes = self.var_name.as_bytes().to_vec();
        name_bytes.extend_from_slice(key.encode_fixed()?.as_ref());

        Ok(Hash::digest(Bytes::from(name_bytes)))
    }

    fn inner_get(&self, key: &K) -> ProtocolResult<Option<V>> {
        if self.keys.inner.contains(key) {
            let mk = self.get_map_key(key)?;
            self.state.borrow().get(&mk)?.map_or_else(
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

    // TODO(@zhounan): Atomicity of insert(k, v) and insert self.keys to
    // ServiceState is not guaranteed for now That must be settled soon after.
    fn inner_insert(&mut self, key: K, value: V) -> ProtocolResult<()> {
        let mk = self.get_map_key(&key)?;

        if !self.contains(&key) {
            self.keys.inner.push(key);
            self.state
                .borrow_mut()
                .insert(self.var_name.clone(), self.keys.encode_fixed()?)?;
        }

        self.state.borrow_mut().insert(mk, value)
    }

    // TODO(@zhounan): Atomicity of insert(k, v) and insert self.keys to
    // ServiceState is not guaranteed for now That must be settled soon after.
    fn inner_remove(&mut self, key: &K) -> ProtocolResult<Option<V>> {
        if self.contains(key) {
            let value: V = self.inner_get(key)?.expect("value should be existed");
            self.keys.inner.remove_item(key);
            self.state
                .borrow_mut()
                .insert(self.var_name.clone(), self.keys.encode_fixed()?)?;

            self.state
                .borrow_mut()
                .insert(self.get_map_key(key)?, Bytes::new())?;

            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
}

impl<S: 'static + ServiceState, K: 'static + FixedCodec + PartialEq, V: 'static + FixedCodec>
    StoreMap<K, V> for DefaultStoreMap<S, K, V>
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
        self.keys.inner.contains(key)
    }

    fn len(&self) -> u32 {
        self.keys.inner.len() as u32
    }

    fn is_empty(&self) -> bool {
        if let 0 = self.len() {
            true
        } else {
            false
        }
    }

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (&K, V)> + 'a> {
        Box::new(MapIter::<S, K, V>::new(0, self))
    }
}

pub struct MapIter<
    'a,
    S: 'static + ServiceState,
    K: 'static + FixedCodec + PartialEq,
    V: 'static + FixedCodec,
> {
    idx: u32,
    map: &'a DefaultStoreMap<S, K, V>,
}

impl<
        'a,
        S: 'static + ServiceState,
        K: 'static + FixedCodec + PartialEq,
        V: 'static + FixedCodec,
    > MapIter<'a, S, K, V>
{
    pub fn new(idx: u32, map: &'a DefaultStoreMap<S, K, V>) -> Self {
        Self { idx, map }
    }
}

impl<
        'a,
        S: 'static + ServiceState,
        K: 'static + FixedCodec + PartialEq,
        V: 'static + FixedCodec,
    > Iterator for MapIter<'a, S, K, V>
{
    type Item = (&'a K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.map.len() {
            let key = self
                .map
                .keys
                .inner
                .get(self.idx as usize)
                .expect("get key should not fail");
            self.idx += 1;
            Some((key, self.map.get(key).expect("get value should not fail")))
        } else {
            None
        }
    }
}
