use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use bytes::Bytes;

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::traits::{ServiceState, StoreMap};
use protocol::types::Hash;
use protocol::ProtocolResult;

use crate::store::StoreError;

pub struct FixedKeys<K: FixedCodec> {
    pub inner: Vec<K>,
}

impl<K: FixedCodec> rlp::Encodable for FixedKeys<K> {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let inner: Vec<Vec<u8>> = self
            .inner
            .iter()
            .map(|k| k.encode_fixed().expect("encode should not fail").to_vec())
            .collect();

        s.begin_list(1).append_list::<Vec<u8>, _>(&inner);
    }
}

impl<K: FixedCodec> rlp::Decodable for FixedKeys<K> {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let inner_u8: Vec<Vec<u8>> = rlp::decode_list(r.at(0)?.as_raw());

        let inner_k: Result<Vec<K>, _> = inner_u8
            .into_iter()
            .map(|v| <_>::decode_fixed(Bytes::from(v)))
            .collect();

        let inner = inner_k.map_err(|_| rlp::DecoderError::Custom("decode K from bytes fail"))?;

        Ok(FixedKeys { inner })
    }
}

impl<K: FixedCodec> FixedCodec for FixedKeys<K> {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}

pub struct DefaultStoreMap<S: ServiceState, K: FixedCodec + PartialEq, V: FixedCodec> {
    state:    Rc<RefCell<S>>,
    var_name: Hash,
    keys:     FixedKeys<K>,
    phantom:  PhantomData<V>,
}

impl<S: ServiceState, K: FixedCodec + PartialEq, V: FixedCodec> DefaultStoreMap<S, K, V> {
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
}

impl<S: ServiceState, K: FixedCodec + PartialEq, V: FixedCodec> StoreMap<K, V>
    for DefaultStoreMap<S, K, V>
{
    fn get(&self, key: &K) -> ProtocolResult<V> {
        if self.keys.inner.contains(key) {
            let mk = self.get_map_key(key)?;
            self.state.borrow().get(&mk)?.map_or_else(
                || <_>::decode_fixed(Bytes::new()).map_err(|_| StoreError::DecodeError.into()),
                |v| Ok(v),
            )
        } else {
            Err(StoreError::GetNone.into())
        }
    }

    fn contains(&self, key: &K) -> ProtocolResult<bool> {
        Ok(self.keys.inner.contains(key))
    }

    // TODO(@zhounan): Atomicity of insert(k, v) and insert self.keys to
    // ServiceState is not guaranteed for now That must be settled soon after.
    fn insert(&mut self, key: K, value: V) -> ProtocolResult<()> {
        let mk = self.get_map_key(&key)?;
        self.state.borrow_mut().insert(mk, value)?;

        if !self.contains(&key)? {
            self.keys.inner.push(key);
            self.state
                .borrow_mut()
                .insert(self.var_name.clone(), self.keys.encode_fixed()?)
        } else {
            Ok(())
        }
    }

    // TODO(@zhounan): Atomicity of insert(k, v) and insert self.keys to
    // ServiceState is not guaranteed for now That must be settled soon after.
    fn remove(&mut self, key: &K) -> ProtocolResult<()> {
        if let true = self.contains(key)? {
            self.keys.inner.remove_item(key);
            self.state
                .borrow_mut()
                .insert(self.var_name.clone(), self.keys.encode_fixed()?)?;

            self.state
                .borrow_mut()
                .insert(self.get_map_key(key)?, Bytes::new())
        } else {
            Err(StoreError::GetNone.into())
        }
    }

    fn len(&self) -> ProtocolResult<usize> {
        Ok(self.keys.inner.len())
    }

    fn for_each<F>(&mut self, mut f: F) -> ProtocolResult<()>
    where
        Self: Sized,
        F: FnMut(&mut V) -> ProtocolResult<()>,
    {
        for k in &self.keys.inner {
            let mut v = self.get(k)?;
            f(&mut v)?;
            let mk = self.get_map_key(k)?;
            self.state.borrow_mut().insert(mk, v)?;
        }

        Ok(())
    }
}
