use std::error::Error;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use rocksdb::{ColumnFamily, Options, WriteBatch, DB};

use protocol::traits::{StorageAdapter, StorageCategory};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[derive(Debug)]
pub struct RocksAdapter {
    db: Arc<DB>,
}

impl RocksAdapter {
    pub fn new(path: String) -> ProtocolResult<Self> {
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

#[async_trait]
impl StorageAdapter for RocksAdapter {
    async fn get(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<Option<Bytes>> {
        let column = get_column(&self.db, c)?;
        let v = self
            .db
            .get_cf(column, key)
            .map_err(RocksAdapterError::from)?;

        Ok(v.map(|v| Bytes::from(v.to_vec())))
    }

    async fn get_batch(
        &self,
        c: StorageCategory,
        keys: Vec<Bytes>,
    ) -> ProtocolResult<Vec<Option<Bytes>>> {
        let column = get_column(&self.db, c)?;

        let mut values = Vec::with_capacity(keys.len());
        for key in keys {
            let v = self
                .db
                .get_cf(column, key)
                .map_err(RocksAdapterError::from)?;

            values.push(v.map(|v| Bytes::from(v.to_vec())));
        }

        Ok(values)
    }

    async fn insert(&self, c: StorageCategory, key: Bytes, value: Bytes) -> ProtocolResult<()> {
        let column = get_column(&self.db, c)?;
        self.db
            .put_cf(column, key.to_vec(), value.to_vec())
            .map_err(RocksAdapterError::from)?;
        Ok(())
    }

    async fn insert_batch(
        &self,
        c: StorageCategory,
        keys: Vec<Bytes>,
        values: Vec<Bytes>,
    ) -> ProtocolResult<()> {
        if keys.len() != values.len() {
            return Err(RocksAdapterError::InsertParameter.into());
        }

        let column = get_column(&self.db, c)?;

        let mut batch = WriteBatch::default();
        for (key, value) in keys.into_iter().zip(values.into_iter()) {
            batch
                .put_cf(column, key, value)
                .map_err(RocksAdapterError::from)?;
        }

        self.db.write(batch).map_err(RocksAdapterError::from)?;
        Ok(())
    }

    async fn contains(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<bool> {
        let column = get_column(&self.db, c)?;
        let v = self
            .db
            .get_cf(column, key)
            .map_err(RocksAdapterError::from)?;

        Ok(v.is_some())
    }

    async fn remove(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<()> {
        let column = get_column(&self.db, c)?;
        self.db
            .delete_cf(column, key)
            .map_err(RocksAdapterError::from)?;
        Ok(())
    }

    async fn remove_batch(&self, c: StorageCategory, keys: Vec<Bytes>) -> ProtocolResult<()> {
        let column = get_column(&self.db, c)?;

        let mut batch = WriteBatch::default();
        for key in keys {
            batch
                .delete_cf(column, key)
                .map_err(RocksAdapterError::from)?;
        }

        self.db.write(batch).map_err(RocksAdapterError::from)?;
        Ok(())
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

fn get_column(db: &DB, c: StorageCategory) -> Result<ColumnFamily, RocksAdapterError> {
    let column = db
        .cf_handle(map_category(c))
        .ok_or_else(|| RocksAdapterError::CategoryNotFound { c })?;
    Ok(column)
}

#[derive(Debug)]
pub enum RocksAdapterError {
    CategoryNotFound { c: StorageCategory },
    RocksDB { error: rocksdb::Error },
    InsertParameter,
}

impl Error for RocksAdapterError {}

impl fmt::Display for RocksAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            RocksAdapterError::CategoryNotFound { c } => format!("category {:?} not found", c),
            RocksAdapterError::RocksDB { error } => format!("rocksdb {:?}", error),
            RocksAdapterError::InsertParameter => "parameters do not match".to_owned(),
        };
        write!(f, "{}", printable)
    }
}

impl From<RocksAdapterError> for ProtocolError {
    fn from(err: RocksAdapterError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
    }
}

impl From<rocksdb::Error> for RocksAdapterError {
    fn from(error: rocksdb::Error) -> Self {
        RocksAdapterError::RocksDB { error }
    }
}

impl From<StorageCategory> for RocksAdapterError {
    fn from(c: StorageCategory) -> Self {
        RocksAdapterError::CategoryNotFound { c }
    }
}
