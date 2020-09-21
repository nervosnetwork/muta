use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use bytes::Bytes;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{ServiceState, StoreArray};
use protocol::types::Hash;
use protocol::ProtocolResult;

use crate::binding::store::FixedKeys;

pub struct DefaultStoreArray<S: ServiceState, E: FixedCodec> {
    state:    Rc<RefCell<S>>,
    var_name: Hash,
    keys:     FixedKeys<Hash>,
    phantom:  PhantomData<E>,
}

impl<S: ServiceState, E: FixedCodec> DefaultStoreArray<S, E> {
    pub fn new(state: Rc<RefCell<S>>, name: &str) -> Self {
        let var_name = Hash::digest(Bytes::from(name.to_owned() + "array"));

        let opt_bs: Option<Bytes> = state
            .borrow()
            .get(&var_name)
            .expect("get array should not fail");

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

    fn inner_get(&self, index: u64) -> ProtocolResult<Option<E>> {
        if let Some(k) = self.keys.inner.get(index as usize) {
            self.state
                .borrow()
                .get(k)?
                .map_or_else(|| Ok(None), |v| Ok(Some(v)))
        } else {
            Ok(None)
        }
    }

    // TODO(@zhounan): Atomicity of insert(k, v) and insert self.keys to
    // ServiceState is not guaranteed for now That must be settled soon after.
    fn inner_push(&mut self, elm: E) -> ProtocolResult<()> {
        let key = Hash::digest(elm.encode_fixed()?);

        self.keys.inner.push(key.clone());
        self.state
            .borrow_mut()
            .insert(self.var_name.clone(), self.keys.encode_fixed()?)?;

        self.state.borrow_mut().insert(key, elm)
    }

    // TODO(@zhounan): Atomicity of insert(k, v) and insert self.keys to
    // ServiceState is not guaranteed for now That must be settled soon after.
    fn inner_remove(&mut self, index: u64) -> ProtocolResult<()> {
        let key = self.keys.inner.remove(index as usize);
        self.state
            .borrow_mut()
            .insert(self.var_name.clone(), self.keys.encode_fixed()?)?;

        self.state.borrow_mut().insert(key, Bytes::new())
    }
}

impl<S: ServiceState, E: FixedCodec> StoreArray<E> for DefaultStoreArray<S, E> {
    fn get(&self, index: u64) -> Option<E> {
        self.inner_get(index)
            .unwrap_or_else(|e| panic!("StoreArray get value failed: {}", e))
    }

    fn push(&mut self, elm: E) {
        self.inner_push(elm)
            .unwrap_or_else(|e| panic!("StoreArray push value failed: {}", e));
    }

    fn remove(&mut self, index: u64) {
        self.inner_remove(index)
            .unwrap_or_else(|e| panic!("StoreArray remove value failed: {}", e));
    }

    fn len(&self) -> u64 {
        self.keys.inner.len() as u64
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (u64, E)> + 'a> {
        Box::new(ArrayIter::<E, Self>::new(0, self))
    }
}

struct ArrayIter<'a, E: FixedCodec, A: StoreArray<E>> {
    idx:     u64,
    array:   &'a A,
    phantom: PhantomData<E>,
}

impl<'a, E: FixedCodec, A: StoreArray<E>> ArrayIter<'a, E, A> {
    pub fn new(idx: u64, array: &'a A) -> Self {
        ArrayIter {
            idx,
            array,
            phantom: PhantomData,
        }
    }
}

impl<'a, E: FixedCodec, A: StoreArray<E>> Iterator for ArrayIter<'a, E, A> {
    type Item = (u64, E);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.array.len() {
            let ele = self
                .array
                .get(self.idx)
                .expect("StoreArray should get Some when index inbound");
            self.idx += 1;
            Some((self.idx - 1, ele))
        } else {
            None
        }
    }
}
