use std::cell::RefCell;
use std::io::Cursor;
use std::mem;
use std::rc::Rc;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use protocol::traits::{ServiceState, StoreBool, StoreString, StoreUint64};
use protocol::types::Hash;
use protocol::ProtocolResult;

use crate::binding::store::StoreError;

pub struct DefaultStoreBool<S: ServiceState> {
    state: Rc<RefCell<S>>,
    key:   Hash,
}

impl<S: ServiceState> DefaultStoreBool<S> {
    pub fn new(state: Rc<RefCell<S>>, var_name: &str) -> Self {
        Self {
            state,
            key: Hash::digest(Bytes::from(var_name.to_owned() + "bool")),
        }
    }
}

impl<S: ServiceState> StoreBool for DefaultStoreBool<S> {
    fn get(&self) -> ProtocolResult<bool> {
        let b: Option<bool> = self.state.borrow().get(&self.key)?;

        b.ok_or(StoreError::GetNone.into())
    }

    fn set(&mut self, b: bool) -> ProtocolResult<()> {
        self.state.borrow_mut().insert(self.key.clone(), b)?;
        Ok(())
    }
}

pub struct DefaultStoreUint64<S: ServiceState> {
    state: Rc<RefCell<S>>,
    key:   Hash,
}

impl<S: ServiceState> DefaultStoreUint64<S> {
    pub fn new(state: Rc<RefCell<S>>, var_name: &str) -> Self {
        Self {
            state,
            key: Hash::digest(Bytes::from(var_name.to_owned() + "uint64")),
        }
    }
}

impl<S: ServiceState> StoreUint64 for DefaultStoreUint64<S> {
    fn get(&self) -> ProtocolResult<u64> {
        let u: Option<u64> = self.state.borrow().get(&self.key)?;

        u.ok_or(StoreError::GetNone.into())
    }

    fn set(&mut self, val: u64) -> ProtocolResult<()> {
        self.state.borrow_mut().insert(self.key.clone(), val)?;
        Ok(())
    }

    // Add val with self
    // And set the result back to self
    fn add(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        match val.overflowing_add(sv) {
            (sum, false) => self.set(sum),
            _ => Err(StoreError::Overflow.into()),
        }
    }

    // Self minus val
    // And set the result back to self
    fn sub(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        if sv >= val {
            self.set(sv - val)
        } else {
            Err(StoreError::Overflow.into())
        }
    }

    // Multiply val with self
    // And set the result back to self
    fn mul(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        match val.overflowing_mul(sv) {
            (mul, false) => self.set(mul),
            _ => Err(StoreError::Overflow.into()),
        }
    }

    // Power of self
    // And set the result back to self
    fn pow(&mut self, val: u32) -> ProtocolResult<()> {
        let sv = self.get()?;

        match sv.overflowing_pow(val) {
            (pow, false) => self.set(pow),
            _ => Err(StoreError::Overflow.into()),
        }
    }

    // Self divided by val
    // And set the result back to self
    fn div(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        if let 0 = val {
            Err(StoreError::Overflow.into())
        } else {
            self.set(sv / val)
        }
    }

    // Remainder of self
    // And set the result back to self
    fn rem(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        if let 0 = val {
            Err(StoreError::Overflow.into())
        } else {
            self.set(sv % val)
        }
    }
}

pub struct DefaultStoreString<S: ServiceState> {
    state: Rc<RefCell<S>>,
    key:   Hash,
}

impl<S: ServiceState> DefaultStoreString<S> {
    pub fn new(state: Rc<RefCell<S>>, var_name: &str) -> Self {
        Self {
            state,
            key: Hash::digest(Bytes::from(var_name.to_owned() + "string")),
        }
    }
}

impl<S: ServiceState> StoreString for DefaultStoreString<S> {
    fn set(&mut self, val: &str) -> ProtocolResult<()> {
        self.state
            .borrow_mut()
            .insert(self.key.clone(), val.to_string())?;
        Ok(())
    }

    fn get(&self) -> ProtocolResult<String> {
        let s: Option<String> = self.state.borrow().get(&self.key)?;

        s.ok_or(StoreError::GetNone.into())
    }

    fn len(&self) -> ProtocolResult<u32> {
        self.get().map(|s| s.len() as u32)
    }

    fn is_empty(&self) -> ProtocolResult<bool> {
        self.get().map(|s| s.is_empty())
    }
}
