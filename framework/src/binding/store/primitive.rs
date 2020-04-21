use std::cell::RefCell;
use std::rc::Rc;

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

    fn get_(&self) -> ProtocolResult<bool> {
        let b: Option<bool> = self.state.borrow().get(&self.key)?;

        match b {
            Some(v) => Ok(v),
            None => {
                self.state.borrow_mut().insert(self.key.clone(), false)?;
                Ok(false)
            }
        }
    }

    fn set_(&mut self, b: bool) -> ProtocolResult<()> {
        self.state.borrow_mut().insert(self.key.clone(), b)?;
        Ok(())
    }
}

impl<S: ServiceState> StoreBool for DefaultStoreBool<S> {
    fn get(&self) -> bool {
        self.get_()
            .unwrap_or_else(|e| panic!("StoreBool get failed: {}", e))
    }

    fn set(&mut self, b: bool) {
        self.set_(b)
            .unwrap_or_else(|e| panic!("StoreBool set failed: {}", e));
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

    fn inner_get(&self) -> ProtocolResult<u64> {
        let u: Option<u64> = self.state.borrow().get(&self.key)?;

        match u {
            Some(v) => Ok(v),
            None => {
                self.state.borrow_mut().insert(self.key.clone(), 0u64)?;
                Ok(0)
            }
        }
    }

    fn inner_set(&mut self, val: u64) -> ProtocolResult<()> {
        self.state.borrow_mut().insert(self.key.clone(), val)?;
        Ok(())
    }

    // Add val with self
    // And set the result back to self
    fn inner_add(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.inner_get()?;

        match val.overflowing_add(sv) {
            (sum, false) => self.inner_set(sum),
            _ => Err(StoreError::Overflow.into()),
        }
    }

    // Self minus val
    // And set the result back to self
    fn inner_sub(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.inner_get()?;

        if sv >= val {
            self.inner_set(sv - val)
        } else {
            Err(StoreError::Overflow.into())
        }
    }

    // Multiply val with self
    // And set the result back to self
    fn inner_mul(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.inner_get()?;

        match val.overflowing_mul(sv) {
            (mul, false) => self.inner_set(mul),
            _ => Err(StoreError::Overflow.into()),
        }
    }

    // Power of self
    // And set the result back to self
    fn inner_pow(&mut self, val: u32) -> ProtocolResult<()> {
        let sv = self.inner_get()?;

        match sv.overflowing_pow(val) {
            (pow, false) => self.inner_set(pow),
            _ => Err(StoreError::Overflow.into()),
        }
    }

    // Self divided by val
    // And set the result back to self
    fn inner_div(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.inner_get()?;

        if let 0 = val {
            Err(StoreError::Overflow.into())
        } else {
            self.inner_set(sv / val)
        }
    }

    // Remainder of self
    // And set the result back to self
    fn inner_rem(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.inner_get()?;

        if let 0 = val {
            Err(StoreError::Overflow.into())
        } else {
            self.inner_set(sv % val)
        }
    }
}

impl<S: ServiceState> StoreUint64 for DefaultStoreUint64<S> {
    fn get(&self) -> u64 {
        self.inner_get()
            .unwrap_or_else(|e| panic!("StoreUint64 get failed: {}", e))
    }

    fn set(&mut self, val: u64) {
        self.inner_set(val)
            .unwrap_or_else(|e| panic!("StoreUint64 set failed: {}", e));
    }

    // Add val with self
    // And set the result back to self
    fn add(&mut self, val: u64) -> bool {
        let mut overflow = false;
        self.inner_add(val)
            .unwrap_or_else(|_| overflow=true );
        overflow
    }

    // Self minus val
    // And set the result back to self
    fn sub(&mut self, val: u64) -> bool {
        let mut overflow = false;
        self.inner_sub(val)
            .unwrap_or_else(|_| overflow=true);
        overflow
    }

    // Multiply val with self
    // And set the result back to self
    fn mul(&mut self, val: u64) -> bool {
        let mut overflow = false;
        self.inner_mul(val)
            .unwrap_or_else(|_| overflow=true);
        overflow
    }

    // Power of self
    // And set the result back to self
    fn pow(&mut self, val: u32) -> bool {
        let mut overflow = false;
        self.inner_pow(val)
            .unwrap_or_else(|_| overflow=true);
        overflow
    }

    // Self divided by val
    // And set the result back to self
    fn div(&mut self, val: u64)  -> bool{
        let mut overflow = false;
        self.inner_div(val)
            .unwrap_or_else(|_| overflow=true);
        overflow
    }

    // Remainder of self
    // And set the result back to self
    fn rem(&mut self, val: u64) -> bool {
        let mut overflow = false;
        self.inner_rem(val)
            .unwrap_or_else(|_| overflow=true);
        overflow
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

    fn set_(&mut self, val: &str) -> ProtocolResult<()> {
        self.state
            .borrow_mut()
            .insert(self.key.clone(), val.to_string())?;
        Ok(())
    }

    fn get_(&self) -> ProtocolResult<String> {
        let s: Option<String> = self.state.borrow().get(&self.key)?;

        match s {
            Some(v) => Ok(v),
            None => {
                self.state
                    .borrow_mut()
                    .insert(self.key.clone(), "".to_string())?;
                Ok("".to_string())
            }
        }
    }

    fn len_(&self) -> ProtocolResult<u32> {
        self.get_().map(|s| s.len() as u32)
    }

    fn is_empty_(&self) -> ProtocolResult<bool> {
        self.get_().map(|s| s.is_empty())
    }
}

impl<S: ServiceState> StoreString for DefaultStoreString<S> {
    fn get(&self) -> String {
        self.get_()
            .unwrap_or_else(|e| panic!("StoreString get failed: {}", e))
    }

    fn set(&mut self, val: &str) {
        self.set_(val)
            .unwrap_or_else(|e| panic!("StoreString set failed: {}", e));
    }

    fn len(&self) -> u32 {
        self.len_()
            .unwrap_or_else(|e| panic!("StoreString get length failed: {}", e))
    }

    fn is_empty(&self) -> bool {
        self.is_empty_()
            .unwrap_or_else(|e| panic!("StoreString get is_empty failed: {}", e))
    }
}
