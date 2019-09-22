use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use derive_more::{Display, From};
use rocksdb::{ColumnFamily, Options, WriteBatch, DB};

use protocol::codec::ProtocolCodec;
use protocol::traits::{StorageAdapter, StorageBatchModify, StorageCategory, StorageSchema};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[derive(Debug)]
pub struct RocksAdapter {
    db: Arc<DB>,
}

impl RocksAdapter {
    pub fn new<P: AsRef<Path>>(path: P) -> ProtocolResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let categories = [
            map_category(StorageCategory::Epoch),
            map_category(StorageCategory::Receipt),
            map_category(StorageCategory::SignedTransaction),
        ];

        let db = DB::open_cf(&opts, path, categories.iter()).map_err(RocksAdapterError::from)?;

        Ok(RocksAdapter { db: Arc::new(db) })
    }
}

macro_rules! db {
    ($db:expr, $op:ident, $column:expr, $key:expr) => {
        $db.$op($column, $key).map_err(RocksAdapterError::from)
    };
    ($db:expr, $op:ident, $column:expr, $key:expr, $val:expr) => {
        $db.$op($column, $key, $val)
            .map_err(RocksAdapterError::from)
    };
}

#[async_trait]
impl StorageAdapter for RocksAdapter {
    async fn insert<S: StorageSchema>(
        &self,
        mut key: <S as StorageSchema>::Key,
        mut val: <S as StorageSchema>::Value,
    ) -> ProtocolResult<()> {
        let column = get_column::<S>(&self.db)?;
        let key = key.encode().await?.to_vec();
        let val = val.encode().await?.to_vec();

        db!(self.db, put_cf, column, key, val)?;

        Ok(())
    }

    async fn get<S: StorageSchema>(
        &self,
        mut key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<Option<<S as StorageSchema>::Value>> {
        let column = get_column::<S>(&self.db)?;
        let key = key.encode().await?;

        let opt_bytes =
            { db!(self.db, get_cf, column, key)?.map(|db_vec| Bytes::from(db_vec.to_vec())) };

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
        let column = get_column::<S>(&self.db)?;
        let key = key.encode().await?.to_vec();

        db!(self.db, delete_cf, column, key)?;

        Ok(())
    }

    async fn contains<S: StorageSchema>(
        &self,
        mut key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<bool> {
        let column = get_column::<S>(&self.db)?;
        let key = key.encode().await?.to_vec();
        let val = db!(self.db, get_cf, column, key)?;

        Ok(val.is_some())
    }

    async fn batch_modify<S: StorageSchema>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
        vals: Vec<StorageBatchModify<S>>,
    ) -> ProtocolResult<()> {
        if keys.len() != vals.len() {
            return Err(RocksAdapterError::BatchLengthMismatch.into());
        }

        let column = get_column::<S>(&self.db)?;
        let mut pairs: Vec<(Bytes, Option<Bytes>)> = Vec::with_capacity(keys.len());

        for (mut key, value) in keys.into_iter().zip(vals.into_iter()) {
            let key = key.encode().await?;

            let value = match value {
                StorageBatchModify::Insert(mut value) => Some(value.encode().await?),
                StorageBatchModify::Remove => None,
            };

            pairs.push((key, value))
        }

        let mut batch = WriteBatch::default();
        for (key, value) in pairs.into_iter() {
            match value {
                Some(value) => db!(batch, put_cf, column, key, value)?,
                None => db!(batch, delete_cf, column, key)?,
            }
        }

        self.db.write(batch).map_err(RocksAdapterError::from)?;
        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum RocksAdapterError {
    #[display(fmt = "category {} not found", _0)]
    CategoryNotFound(&'static str),

    #[display(fmt = "rocksdb {}", _0)]
    RocksDB(rocksdb::Error),

    #[display(fmt = "parameters do not match")]
    InsertParameter,

    #[display(fmt = "batch length dont match")]
    BatchLengthMismatch,
}

impl Error for RocksAdapterError {}

impl From<RocksAdapterError> for ProtocolError {
    fn from(err: RocksAdapterError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
    }
}

const C_EPOCHS: &str = "c1";
const C_SIGNED_TRANSACTIONS: &str = "c2";
const C_RECEIPTS: &str = "c3";

fn map_category(c: StorageCategory) -> &'static str {
    match c {
        StorageCategory::Epoch => C_EPOCHS,
        StorageCategory::Receipt => C_RECEIPTS,
        StorageCategory::SignedTransaction => C_SIGNED_TRANSACTIONS,
    }
}

fn get_column<S: StorageSchema>(db: &DB) -> Result<ColumnFamily, RocksAdapterError> {
    let category = map_category(S::category());

    let column = db
        .cf_handle(category)
        .ok_or_else(|| RocksAdapterError::from(category))?;

    Ok(column)
}
