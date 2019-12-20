use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use bytes::Bytes;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{ServiceState, StoreArray};
use protocol::types::Hash;
use protocol::ProtocolResult;

use crate::store::FixedKeys;
use crate::store::StoreError;

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
}

impl<S: ServiceState, E: FixedCodec> StoreArray<E> for DefaultStoreArray<S, E> {
    fn get(&self, index: usize) -> ProtocolResult<E> {
        if let Some(k) = self.keys.inner.get(index) {
            self.state.borrow().get(k)?.map_or_else(
                || <_>::decode_fixed(Bytes::new()).map_err(|_| StoreError::DecodeError.into()),
                Ok,
            )
        } else {
            Err(StoreError::OutRange.into())
        }
    }

    // TODO(@zhounan): Atomicity of insert(k, v) and insert self.keys to
    // ServiceState is not guaranteed for now That must be settled soon after.
    fn push(&mut self, elm: E) -> ProtocolResult<()> {
        let key = Hash::digest(elm.encode_fixed()?);

        self.keys.inner.push(key.clone());
        self.state
            .borrow_mut()
            .insert(self.var_name.clone(), self.keys.encode_fixed()?)?;

        self.state.borrow_mut().insert(key, elm)
    }

    // TODO(@zhounan): Atomicity of insert(k, v) and insert self.keys to
    // ServiceState is not guaranteed for now That must be settled soon after.
    fn remove(&mut self, index: usize) -> ProtocolResult<()> {
        let key = self.keys.inner.remove(index);
        self.state
            .borrow_mut()
            .insert(self.var_name.clone(), self.keys.encode_fixed()?)?;

        self.state.borrow_mut().insert(key, Bytes::new())
    }

    fn len(&self) -> ProtocolResult<usize> {
        Ok(self.keys.inner.len())
    }

    fn is_empty(&self) -> ProtocolResult<bool> {
        if let 0 = self.len()? {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // TODO(@zhounan): If element was not changed by f, then it should not be
    // inserted to ServiceState for performance reason
    fn for_each<F>(&mut self, mut f: F) -> ProtocolResult<()>
    where
        Self: Sized,
        F: FnMut(&mut E) -> ProtocolResult<()>,
    {
        for key in &self.keys.inner {
            let mut elm: E = self
                .state
                .borrow()
                .get(key)?
                .map_or_else::<ProtocolResult<E>, _, _>(
                    || <_>::decode_fixed(Bytes::new()).map_err(|_| StoreError::DecodeError.into()),
                    Ok,
                )?;

            f(&mut elm)?;

            self.state.borrow_mut().insert(key.clone(), elm)?;
        }

        Ok(())
    }
}
