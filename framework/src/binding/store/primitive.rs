use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;

use protocol::traits::{ServiceState, StoreBool, StoreString, StoreUint64};
use protocol::types::Hash;
use protocol::ProtocolResult;

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

    fn inner_get(&self) -> ProtocolResult<bool> {
        let b: Option<bool> = self.state.borrow().get(&self.key)?;

        match b {
            Some(v) => Ok(v),
            None => {
                self.state.borrow_mut().insert(self.key.clone(), false)?;
                Ok(false)
            }
        }
    }

    fn inner_set(&mut self, b: bool) -> ProtocolResult<()> {
        self.state.borrow_mut().insert(self.key.clone(), b)?;
        Ok(())
    }
}

impl<S: ServiceState> StoreBool for DefaultStoreBool<S> {
    fn get(&self) -> bool {
        self.inner_get()
            .unwrap_or_else(|e| panic!("StoreBool get failed: {}", e))
    }

    fn set(&mut self, b: bool) {
        self.inner_set(b)
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

    fn inner_get(&self) -> u64 {
        let u: Option<u64> = self
            .state
            .borrow()
            .get(&self.key)
            .unwrap_or_else(|e| panic!("StoreUint64 get failed: {}", e));

        match u {
            Some(v) => v,
            None => {
                self.state
                    .borrow_mut()
                    .insert(self.key.clone(), 0u64)
                    .unwrap_or_else(|e| panic!("StoreUint64 get failed: {}", e));
                0
            }
        }
    }

    fn inner_set(&mut self, val: u64) {
        self.state
            .borrow_mut()
            .insert(self.key.clone(), val)
            .unwrap_or_else(|e| panic!("StoreUint64 set failed: {}", e));
    }

    // Add val with self
    // And set the result back to self
    fn inner_add(&mut self, val: u64) -> bool {
        let sv = self.inner_get();

        match val.overflowing_add(sv) {
            (sum, false) => {
                self.inner_set(sum);
                false
            }
            _ => true,
        }
    }

    // Self minus val
    // And set the result back to self
    fn inner_sub(&mut self, val: u64) -> bool {
        let sv = self.inner_get();

        if sv >= val {
            self.inner_set(sv - val);
            false
        } else {
            true
        }
    }

    // Multiply val with self
    // And set the result back to self
    fn inner_mul(&mut self, val: u64) -> bool {
        let sv = self.inner_get();

        match val.overflowing_mul(sv) {
            (mul, false) => {
                self.inner_set(mul);
                false
            }
            _ => true,
        }
    }

    // Power of self
    // And set the result back to self
    fn inner_pow(&mut self, val: u32) -> bool {
        let sv = self.inner_get();

        match sv.overflowing_pow(val) {
            (pow, false) => {
                self.inner_set(pow);
                false
            }
            _ => true,
        }
    }

    // Self divided by val
    // And set the result back to self
    fn inner_div(&mut self, val: u64) -> bool {
        let sv = self.inner_get();

        if let 0 = val {
            true
        } else {
            self.inner_set(sv / val);
            false
        }
    }

    // Remainder of self
    // And set the result back to self
    fn inner_rem(&mut self, val: u64) -> bool {
        let sv = self.inner_get();

        if let 0 = val {
            true
        } else {
            self.inner_set(sv % val);
            false
        }
    }
}

impl<S: ServiceState> StoreUint64 for DefaultStoreUint64<S> {
    fn get(&self) -> u64 {
        self.inner_get()
    }

    fn set(&mut self, val: u64) {
        self.inner_set(val);
    }

    // Add val with self
    // And set the result back to self
    fn safe_add(&mut self, val: u64) -> bool {
        self.inner_add(val)
    }

    // Self minus val
    // And set the result back to self
    fn safe_sub(&mut self, val: u64) -> bool {
        self.inner_sub(val)
    }

    // Multiply val with self
    // And set the result back to self
    fn safe_mul(&mut self, val: u64) -> bool {
        self.inner_mul(val)
    }

    // Power of self
    // And set the result back to self
    fn safe_pow(&mut self, val: u32) -> bool {
        self.inner_pow(val)
    }

    // Self divided by val
    // And set the result back to self
    fn safe_div(&mut self, val: u64) -> bool {
        self.inner_div(val)
    }

    // Remainder of self
    // And set the result back to self
    fn safe_rem(&mut self, val: u64) -> bool {
        self.inner_rem(val)
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

    fn inner_set(&mut self, val: &str) -> ProtocolResult<()> {
        self.state
            .borrow_mut()
            .insert(self.key.clone(), val.to_string())?;
        Ok(())
    }

    fn inner_get(&self) -> ProtocolResult<String> {
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

    fn inner_len(&self) -> ProtocolResult<u64> {
        self.inner_get().map(|s| s.len() as u64)
    }

    fn is_empty_(&self) -> ProtocolResult<bool> {
        self.inner_get().map(|s| s.is_empty())
    }
}

impl<S: ServiceState> StoreString for DefaultStoreString<S> {
    fn get(&self) -> String {
        self.inner_get()
            .unwrap_or_else(|e| panic!("StoreString get failed: {}", e))
    }

    fn set(&mut self, val: &str) {
        self.inner_set(val)
            .unwrap_or_else(|e| panic!("StoreString set failed: {}", e));
    }

    fn len(&self) -> u64 {
        self.inner_len()
            .unwrap_or_else(|e| panic!("StoreString get length failed: {}", e))
    }

    fn is_empty(&self) -> bool {
        self.is_empty_()
            .unwrap_or_else(|e| panic!("StoreString get is_empty failed: {}", e))
    }
}
