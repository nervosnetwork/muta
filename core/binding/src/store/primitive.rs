use std::io::Cursor;
use std::mem;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use cita_trie::DB as TrieDB;

use protocol::traits::{ServiceState, StoreBool, StoreString, StoreUint64};
use protocol::types::Hash;
use protocol::ProtocolResult;

use crate::state::GeneralServiceState;
use crate::store::StoreType;
use crate::BindingError;

pub struct DefaultStoreBool<DB: TrieDB> {
    state: GeneralServiceState<DB>,
    key:   Hash,
}

impl<DB: TrieDB> DefaultStoreBool<DB> {
    pub fn new(state: GeneralServiceState<DB>, var_name: &str) -> Self {
        Self {
            state,
            key: Hash::digest(Bytes::from(var_name.to_owned() + "bool")),
        }
    }
}

impl<DB: TrieDB> StoreBool for DefaultStoreBool<DB> {
    fn get(&self) -> ProtocolResult<bool> {
        let opt_bs: Option<Bytes> = self.state.get(&self.key)?;

        match opt_bs {
            Some(bs) => {
                let mut rdr = Cursor::new(bs.to_vec());
                let u = rdr.read_u8().unwrap();
                match u {
                    0 => Ok(false),
                    1 => Ok(true),
                    _ => Err(BindingError::Store(StoreType::DecodeError).into()),
                }
            }
            None => Err(BindingError::Store(StoreType::GetNone).into()),
        }
    }

    fn set(&mut self, b: bool) -> ProtocolResult<()> {
        let bs = match b {
            true => [1u8; mem::size_of::<u8>()],
            false => [0u8; mem::size_of::<u8>()],
        };

        let val = Bytes::from(bs.as_ref());
        self.state.insert(self.key.clone(), val)?;
        Ok(())
    }
}

pub struct DefaultStoreUint64<DB: TrieDB> {
    state: GeneralServiceState<DB>,
    key:   Hash,
}

impl<DB: TrieDB> DefaultStoreUint64<DB> {
    pub fn new(state: GeneralServiceState<DB>, var_name: &str) -> Self {
        Self {
            state,
            key: Hash::digest(Bytes::from(var_name.to_owned() + "uint64")),
        }
    }
}

impl<DB: TrieDB> StoreUint64 for DefaultStoreUint64<DB> {
    fn get(&self) -> ProtocolResult<u64> {
        let opt_bs: Option<Bytes> = self.state.get(&self.key)?;

        match opt_bs {
            Some(bs) => {
                let mut rdr = Cursor::new(bs.to_vec());
                Ok(rdr.read_u64::<BigEndian>().unwrap())
            }
            None => Err(BindingError::Store(StoreType::GetNone).into()),
        }
    }

    fn set(&mut self, val: u64) -> ProtocolResult<()> {
        let mut bs = [0u8; mem::size_of::<u64>()];
        bs.as_mut()
            .write_u64::<BigEndian>(val)
            .expect("Unable to write");
        let val = Bytes::from(bs.as_ref());

        self.state.insert(self.key.clone(), val)?;
        Ok(())
    }

    // Add val with self
    // And set the result back to self
    fn add(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        match val.overflowing_add(sv) {
            (sum, false) => self.set(sum),
            _ => Err(BindingError::Store(StoreType::Overflow).into()),
        }
    }

    // Self minus val
    // And set the result back to self
    fn sub(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        match sv >= val {
            true => self.set(sv - val),
            false => Err(BindingError::Store(StoreType::Overflow).into()),
        }
    }

    // Multiply val with self
    // And set the result back to self
    fn mul(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        match val.overflowing_mul(sv) {
            (mul, false) => self.set(mul),
            _ => Err(BindingError::Store(StoreType::Overflow).into()),
        }
    }

    // Power of self
    // And set the result back to self
    fn pow(&mut self, val: u32) -> ProtocolResult<()> {
        let sv = self.get()?;

        match sv.overflowing_pow(val) {
            (pow, false) => self.set(pow),
            _ => Err(BindingError::Store(StoreType::Overflow).into()),
        }
    }

    // Self divided by val
    // And set the result back to self
    fn div(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        if (0 == val) {
            Err(BindingError::Store(StoreType::Overflow).into())
        } else {
            self.set(sv / val)
        }
    }

    // Remainder of self
    // And set the result back to self
    fn rem(&mut self, val: u64) -> ProtocolResult<()> {
        let sv = self.get()?;

        if (0 == val) {
            Err(BindingError::Store(StoreType::Overflow).into())
        } else {
            self.set(sv % val)
        }
    }
}

pub struct DefaultStoreString<DB: TrieDB> {
    state: GeneralServiceState<DB>,
    key:   Hash,
}

impl<DB: TrieDB> DefaultStoreString<DB> {
    pub fn new(state: GeneralServiceState<DB>, var_name: &str) -> Self {
        Self {
            state,
            key: Hash::digest(Bytes::from(var_name.to_owned() + "string")),
        }
    }
}

impl<DB: TrieDB> StoreString for DefaultStoreString<DB> {
    fn set(&mut self, val: &str) -> ProtocolResult<()> {
        let val = Bytes::from(val);

        self.state.insert(self.key.clone(), val)?;
        Ok(())
    }

    fn get(&self) -> ProtocolResult<String> {
        let opt_bs: Option<Bytes> = self.state.get(&self.key)?;

        match opt_bs {
            Some(bs) => Ok(String::from_utf8(bs.to_vec()).unwrap()),
            None => Err(BindingError::Store(StoreType::GetNone).into()),
        }
    }

    fn len(&self) -> ProtocolResult<usize> {
        self.get().map(|s| s.len())
    }

    fn is_empty(&self) -> ProtocolResult<bool> {
        self.get().map(|s| s.is_empty())
    }
}
