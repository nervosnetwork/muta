use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use derive_more::{Display, From};
use parking_lot::RwLock;

use protocol::codec::ProtocolCodec;
use protocol::traits::{StorageAdapter, StorageBatchModify, StorageSchema};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[derive(Debug)]
pub struct MemoryAdapter {
    db: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryAdapter {
    pub fn new() -> Self {
        MemoryAdapter {
            db: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryAdapter {
    fn default() -> Self {
        MemoryAdapter {
            db: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl StorageAdapter for MemoryAdapter {
    async fn insert<S: StorageSchema>(
        &self,
        mut key: <S as StorageSchema>::Key,
        mut val: <S as StorageSchema>::Value,
    ) -> ProtocolResult<()> {
        let key = key.encode().await?.to_vec();
        let val = val.encode().await?.to_vec();

        self.db.write().insert(key, val);

        Ok(())
    }

    async fn get<S: StorageSchema>(
        &self,
        mut key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<Option<<S as StorageSchema>::Value>> {
        let key = key.encode().await?;

        let opt_bytes = self.db.read().get(&key.to_vec()).cloned();

        if let Some(bytes) = opt_bytes {
            let val = <_>::decode(bytes).await?;

            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    async fn remove<S: StorageSchema>(
        &self,
        mut key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<()> {
        let key = key.encode().await?.to_vec();

        self.db.write().remove(&key);

        Ok(())
    }

    async fn contains<S: StorageSchema>(
        &self,
        mut key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<bool> {
        let key = key.encode().await?.to_vec();

        Ok(self.db.read().get(&key).is_some())
    }

    async fn batch_modify<S: StorageSchema>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
        vals: Vec<StorageBatchModify<S>>,
    ) -> ProtocolResult<()> {
        if keys.len() != vals.len() {
            return Err(MemoryAdapterError::BatchLengthMismatch.into());
        }

        let mut pairs: Vec<(Bytes, Option<Bytes>)> = Vec::with_capacity(keys.len());

        for (mut key, value) in keys.into_iter().zip(vals.into_iter()) {
            let key = key.encode().await?;

            let value = match value {
                StorageBatchModify::Insert(mut value) => Some(value.encode().await?),
                StorageBatchModify::Remove => None,
            };

            pairs.push((key, value))
        }

        for (key, value) in pairs.into_iter() {
            match value {
                Some(value) => self.db.write().insert(key.to_vec(), value.to_vec()),
                None => self.db.write().remove(&key.to_vec()),
            };
        }

        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum MemoryAdapterError {
    #[display(fmt = "batch length dont match")]
    BatchLengthMismatch,
}

impl Error for MemoryAdapterError {}

impl From<MemoryAdapterError> for ProtocolError {
    fn from(err: MemoryAdapterError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
    }
}
