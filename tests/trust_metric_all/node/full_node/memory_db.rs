use derive_more::Display;
use parking_lot::RwLock;
use protocol::{
    async_trait,
    codec::ProtocolCodecSync,
    traits::{
        IntoIteratorByRef, StorageAdapter, StorageBatchModify, StorageIterator, StorageSchema,
    },
    Bytes, ProtocolError, ProtocolErrorKind, ProtocolResult,
};

use std::{
    collections::{hash_map, HashMap},
    marker::PhantomData,
    ops::Deref,
    sync::Arc,
};

#[derive(Debug, Display)]
pub enum MemoryDBError {
    #[display(fmt = "batch length dont match")]
    BatchLengthMismatch,
}

impl std::error::Error for MemoryDBError {}

impl From<MemoryDBError> for ProtocolError {
    fn from(err: MemoryDBError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
    }
}

type Category = HashMap<Vec<u8>, Vec<u8>>;

#[derive(Clone)]
pub struct MemoryDB {
    trie: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    db:   Arc<RwLock<HashMap<String, Category>>>,
}

impl Default for MemoryDB {
    fn default() -> Self {
        MemoryDB {
            trie: Default::default(),
            db:   Default::default(),
        }
    }
}

impl Deref for MemoryDB {
    type Target = Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>;

    fn deref(&self) -> &Self::Target {
        &self.trie
    }
}

impl cita_trie::DB for MemoryDB {
    type Error = MemoryDBError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.read().get(key).cloned())
    }

    fn contains(&self, key: &[u8]) -> Result<bool, Self::Error> {
        Ok(self.read().contains_key(key))
    }

    fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Self::Error> {
        self.write().insert(key, value);
        Ok(())
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        if keys.len() != values.len() {
            return Err(MemoryDBError::BatchLengthMismatch);
        }

        for (key, value) in keys.into_iter().zip(values.into_iter()) {
            self.write().insert(key, value);
        }
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        self.write().remove(key);
        Ok(())
    }

    fn remove_batch(&self, keys: &[Vec<u8>]) -> Result<(), Self::Error> {
        for key in keys {
            self.write().remove(key);
        }

        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct MemoryIterator<'a, S: StorageSchema> {
    inner: hash_map::Iter<'a, Vec<u8>, Vec<u8>>,
    pin_s: PhantomData<S>,
}

impl<'a, S: StorageSchema> Iterator for MemoryIterator<'a, S> {
    type Item = ProtocolResult<(<S as StorageSchema>::Key, <S as StorageSchema>::Value)>;

    fn next(&mut self) -> Option<Self::Item> {
        let kv_decode = |(k_bytes, v_bytes): (&Vec<u8>, &Vec<u8>)| -> ProtocolResult<_> {
            let k_bytes = Bytes::copy_from_slice(k_bytes.as_ref());
            let key = <_>::decode_sync(k_bytes)?;

            let v_bytes = Bytes::copy_from_slice(&v_bytes.as_ref());
            let val = <_>::decode_sync(v_bytes)?;

            Ok((key, val))
        };

        self.inner.next().map(kv_decode)
    }
}

pub struct MemoryIntoIterator<'a, S: StorageSchema> {
    inner: parking_lot::RwLockReadGuard<'a, HashMap<String, Category>>,
    pin_s: PhantomData<S>,
}

impl<'a, 'b: 'a, S: StorageSchema> IntoIterator for &'b MemoryIntoIterator<'a, S> {
    type IntoIter = StorageIterator<'a, S>;
    type Item = ProtocolResult<(<S as StorageSchema>::Key, <S as StorageSchema>::Value)>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(MemoryIterator {
            inner: self
                .inner
                .get(&S::category().to_string())
                .expect("impossible, already ensure we have category in prepare_iter")
                .iter(),
            pin_s: PhantomData::<S>,
        })
    }
}

impl<'c, S: StorageSchema> IntoIteratorByRef<S> for MemoryIntoIterator<'c, S> {
    fn ref_to_iter<'a, 'b: 'a>(&'b self) -> StorageIterator<'a, S> {
        self.into_iter()
    }
}

#[async_trait]
impl StorageAdapter for MemoryDB {
    async fn insert<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
        val: <S as StorageSchema>::Value,
    ) -> ProtocolResult<()> {
        let key = key.encode_sync()?.to_vec();
        let val = val.encode_sync()?.to_vec();

        let mut db = self.db.write();
        let db = db
            .entry(S::category().to_string())
            .or_insert_with(HashMap::new);

        db.insert(key, val);

        Ok(())
    }

    async fn get<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<Option<<S as StorageSchema>::Value>> {
        let key = key.encode_sync()?;

        let mut db = self.db.write();
        let db = db
            .entry(S::category().to_string())
            .or_insert_with(HashMap::new);

        let opt_bytes = db.get(&key.to_vec()).cloned();

        if let Some(bytes) = opt_bytes {
            let val = <_>::decode_sync(Bytes::copy_from_slice(&bytes))?;

            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    async fn remove<S: StorageSchema>(&self, key: <S as StorageSchema>::Key) -> ProtocolResult<()> {
        let key = key.encode_sync()?.to_vec();

        let mut db = self.db.write();
        let db = db
            .entry(S::category().to_string())
            .or_insert_with(HashMap::new);

        db.remove(&key);

        Ok(())
    }

    async fn contains<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<bool> {
        let key = key.encode_sync()?.to_vec();

        let mut db = self.db.write();
        let db = db
            .entry(S::category().to_string())
            .or_insert_with(HashMap::new);

        Ok(db.get(&key).is_some())
    }

    async fn batch_modify<S: StorageSchema>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
        vals: Vec<StorageBatchModify<S>>,
    ) -> ProtocolResult<()> {
        if keys.len() != vals.len() {
            return Err(MemoryDBError::BatchLengthMismatch.into());
        }

        let mut pairs: Vec<(Bytes, Option<Bytes>)> = Vec::with_capacity(keys.len());

        for (key, value) in keys.into_iter().zip(vals.into_iter()) {
            let key = key.encode_sync()?;

            let value = match value {
                StorageBatchModify::Insert(value) => Some(value.encode_sync()?),
                StorageBatchModify::Remove => None,
            };

            pairs.push((key, value))
        }

        let mut db = self.db.write();
        let db = db
            .entry(S::category().to_string())
            .or_insert_with(HashMap::new);

        for (key, value) in pairs.into_iter() {
            match value {
                Some(value) => db.insert(key.to_vec(), value.to_vec()),
                None => db.remove(&key.to_vec()),
            };
        }

        Ok(())
    }

    fn prepare_iter<'a, 'b: 'a, S: StorageSchema + 'static, P: AsRef<[u8]> + 'a>(
        &'b self,
        _prefix: &P,
    ) -> ProtocolResult<Box<dyn IntoIteratorByRef<S> + 'a>> {
        {
            self.db
                .write()
                .entry(S::category().to_string())
                .or_insert_with(HashMap::new);
        }

        Ok(Box::new(MemoryIntoIterator {
            inner: self.db.read(),
            pin_s: PhantomData::<S>,
        }))
    }
}
