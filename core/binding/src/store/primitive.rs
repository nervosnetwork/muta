use std::cell::RefCell;
use std::io::Cursor;
use std::mem;
use std::rc::Rc;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use protocol::traits::{ServiceState, StoreBool, StoreString, StoreUint64};
use protocol::types::Hash;
use protocol::ProtocolResult;

use crate::store::StoreError;

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
        let bs: Bytes = self
            .state
            .borrow()
            .get(&self.key)?
            .ok_or(StoreError::GetNone)?;

        let mut rdr = Cursor::new(bs.to_vec());
        let u = rdr.read_u8().expect("read u8 should not fail");
        match u {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(StoreError::DecodeError.into()),
        }
    }

    fn set(&mut self, b: bool) -> ProtocolResult<()> {
        let bs = match b {
            true => [1u8; mem::size_of::<u8>()],
            false => [0u8; mem::size_of::<u8>()],
        };

        let val = Bytes::from(bs.as_ref());
        self.state.borrow_mut().insert(self.key.clone(), val)?;
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
        let bs: Bytes = self
            .state
            .borrow()
            .get(&self.key)?
            .ok_or(StoreError::GetNone)?;
        let mut rdr = Cursor::new(bs.to_vec());

        Ok(rdr
            .read_u64::<LittleEndian>()
            .expect("read u64 should not fail"))
    }

    fn set(&mut self, val: u64) -> ProtocolResult<()> {
        let mut bs = [0u8; mem::size_of::<u64>()];
        bs.as_mut()
            .write_u64::<LittleEndian>(val)
            .expect("write u64 should not fail");
        let val = Bytes::from(bs.as_ref());

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

        match sv >= val {
            true => self.set(sv - val),
            false => Err(StoreError::Overflow.into()),
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

        if (0 == val) {
            Err(StoreError::Overflow.into())
        } else {
            self.set(sv / val)
        }
    }

    // Remainder of self
    // And set the result back to self
    fn rem(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        if (0 == val) {
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
        let val = Bytes::from(val);

        self.state.borrow_mut().insert(self.key.clone(), val)?;
        Ok(())
    }

    fn get(&self) -> ProtocolResult<String> {
        let bs: Bytes = self
            .state
            .borrow()
            .get(&self.key)?
            .ok_or(StoreError::GetNone)?;

        Ok(String::from_utf8(bs.to_vec()).expect("get string should not fail"))
    }

    fn len(&self) -> ProtocolResult<usize> {
        self.get().map(|s| s.len())
    }

    fn is_empty(&self) -> ProtocolResult<bool> {
        self.get().map(|s| s.is_empty())
    }
}
