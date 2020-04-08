use derive_more::Display;
use protocol::{
    async_trait,
    codec::ProtocolCodecSync,
    traits::{StorageAdapter, StorageBatchModify, StorageSchema},
    Bytes, ProtocolError, ProtocolErrorKind, ProtocolResult,
};

use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, RwLock},
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

#[derive(Clone)]
pub struct MemoryDB(Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>);

impl Default for MemoryDB {
    fn default() -> Self {
        MemoryDB(Default::default())
    }
}

impl Deref for MemoryDB {
    type Target = Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl cita_trie::DB for MemoryDB {
    type Error = MemoryDBError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.read().unwrap().get(key).cloned())
    }

    fn contains(&self, key: &[u8]) -> Result<bool, Self::Error> {
        Ok(self.read().unwrap().contains_key(key))
    }

    fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Self::Error> {
        self.write().unwrap().insert(key, value);
        Ok(())
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        if keys.len() != values.len() {
            return Err(MemoryDBError::BatchLengthMismatch);
        }

        for (key, value) in keys.into_iter().zip(values.into_iter()) {
            self.write().unwrap().insert(key, value);
        }
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        self.write().unwrap().remove(key);
        Ok(())
    }

    fn remove_batch(&self, keys: &[Vec<u8>]) -> Result<(), Self::Error> {
        for key in keys {
            self.write().unwrap().remove(key);
        }

        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        Ok(())
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

        self.write().unwrap().insert(key, val);
        Ok(())
    }

    async fn get<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<Option<<S as StorageSchema>::Value>> {
        let key = key.encode_sync()?;
        let opt_bytes = self.read().unwrap().get(&key.to_vec()).cloned();

        if let Some(bytes) = opt_bytes {
            let val = <_>::decode_sync(Bytes::from(bytes))?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    async fn remove<S: StorageSchema>(&self, key: <S as StorageSchema>::Key) -> ProtocolResult<()> {
        let key = key.encode_sync()?.to_vec();

        self.write().unwrap().remove(&key);

        Ok(())
    }

    async fn contains<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<bool> {
        let key = key.encode_sync()?.to_vec();

        Ok(self.read().unwrap().get(&key).is_some())
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

        for (key, value) in pairs.into_iter() {
            match value {
                Some(value) => self.write().unwrap().insert(key.to_vec(), value.to_vec()),
                None => self.write().unwrap().remove(&key.to_vec()),
            };
        }

        Ok(())
    }
}
