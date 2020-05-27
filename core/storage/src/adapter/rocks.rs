use std::error::Error;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use derive_more::{Display, From};
use rocksdb::{ColumnFamily, DBIterator, Options, WriteBatch, DB};

use protocol::codec::ProtocolCodecSync;
use protocol::traits::{
    IntoIteratorByRef, StorageAdapter, StorageBatchModify, StorageCategory, StorageIterator,
    StorageSchema,
};
use protocol::Bytes;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[derive(Debug)]
pub struct RocksAdapter {
    db: Arc<DB>,
}

impl RocksAdapter {
    pub fn new<P: AsRef<Path>>(path: P, max_open_files: i32) -> ProtocolResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_max_open_files(max_open_files);

        let categories = [
            map_category(StorageCategory::Block),
            map_category(StorageCategory::Receipt),
            map_category(StorageCategory::SignedTransaction),
            map_category(StorageCategory::Wal),
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

pub struct RocksIterator<'a, S: StorageSchema> {
    inner: DBIterator<'a>,
    pin_s: PhantomData<S>,
}

impl<'a, S: StorageSchema> Iterator for RocksIterator<'a, S> {
    type Item = ProtocolResult<(<S as StorageSchema>::Key, <S as StorageSchema>::Value)>;

    fn next(&mut self) -> Option<Self::Item> {
        let kv_decode = |(k_bytes, v_bytes): (Box<[u8]>, Box<[u8]>)| -> ProtocolResult<_> {
            let k_bytes = Bytes::copy_from_slice(k_bytes.as_ref());
            let key = <_>::decode_sync(k_bytes)?;

            let v_bytes = Bytes::copy_from_slice(&v_bytes.as_ref());
            let val = <_>::decode_sync(v_bytes)?;

            Ok((key, val))
        };

        self.inner.next().map(kv_decode)
    }
}

pub struct RocksIntoIterator<'a, S: StorageSchema, P: AsRef<[u8]>> {
    db:     Arc<DB>,
    column: &'a ColumnFamily,
    prefix: &'a P,
    pin_s:  PhantomData<S>,
}

impl<'a, 'b: 'a, S: StorageSchema, P: AsRef<[u8]>> IntoIterator
    for &'b RocksIntoIterator<'a, S, P>
{
    type IntoIter = StorageIterator<'a, S>;
    type Item = ProtocolResult<(<S as StorageSchema>::Key, <S as StorageSchema>::Value)>;

    fn into_iter(self) -> Self::IntoIter {
        let iter: DBIterator<'_> = self.db.prefix_iterator_cf(self.column, self.prefix);

        Box::new(RocksIterator {
            inner: iter,
            pin_s: PhantomData::<S>,
        })
    }
}

impl<'c, S: StorageSchema, P: AsRef<[u8]>> IntoIteratorByRef<S> for RocksIntoIterator<'c, S, P> {
    fn ref_to_iter<'a, 'b: 'a>(&'b self) -> StorageIterator<'a, S> {
        self.into_iter()
    }
}

#[async_trait]
impl StorageAdapter for RocksAdapter {
    async fn insert<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
        val: <S as StorageSchema>::Value,
    ) -> ProtocolResult<()> {
        let column = get_column::<S>(&self.db)?;
        let key = key.encode_sync()?.to_vec();
        let val = val.encode_sync()?.to_vec();

        db!(self.db, put_cf, column, key, val)?;

        Ok(())
    }

    async fn get<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<Option<<S as StorageSchema>::Value>> {
        let column = get_column::<S>(&self.db)?;
        let key = key.encode_sync()?;

        let opt_bytes =
            { db!(self.db, get_cf, column, key)?.map(|db_vec| Bytes::copy_from_slice(&db_vec)) };

        if let Some(bytes) = opt_bytes {
            let val = <_>::decode_sync(bytes)?;

            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    async fn remove<S: StorageSchema>(&self, key: <S as StorageSchema>::Key) -> ProtocolResult<()> {
        let column = get_column::<S>(&self.db)?;
        let key = key.encode_sync()?.to_vec();

        db!(self.db, delete_cf, column, key)?;

        Ok(())
    }

    async fn contains<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<bool> {
        let column = get_column::<S>(&self.db)?;
        let key = key.encode_sync()?.to_vec();
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

        for (key, value) in keys.into_iter().zip(vals.into_iter()) {
            let key = key.encode_sync()?;

            let value = match value {
                StorageBatchModify::Insert(value) => Some(value.encode_sync()?),
                StorageBatchModify::Remove => None,
            };

            pairs.push((key, value))
        }

        let mut batch = WriteBatch::default();
        for (key, value) in pairs.into_iter() {
            match value {
                Some(value) => batch.put_cf(column, key, value),
                None => batch.delete_cf(column, key),
            }
        }

        self.db.write(batch).map_err(RocksAdapterError::from)?;
        Ok(())
    }

    fn prepare_iter<'a, 'b: 'a, S: StorageSchema + 'static, P: AsRef<[u8]> + 'a>(
        &'b self,
        prefix: &'a P,
    ) -> ProtocolResult<Box<dyn IntoIteratorByRef<S> + 'a>> {
        let column = get_column::<S>(&self.db)?;

        let rocks_iter = RocksIntoIterator {
            db: Arc::clone(&self.db),
            column,
            prefix,
            pin_s: PhantomData::<S>,
        };
        Ok(Box::new(rocks_iter))
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

const C_BLOCKS: &str = "c1";
const C_SIGNED_TRANSACTIONS: &str = "c2";
const C_RECEIPTS: &str = "c3";
const C_WALS: &str = "c4";

fn map_category(c: StorageCategory) -> &'static str {
    match c {
        StorageCategory::Block => C_BLOCKS,
        StorageCategory::Receipt => C_RECEIPTS,
        StorageCategory::SignedTransaction => C_SIGNED_TRANSACTIONS,
        StorageCategory::Wal => C_WALS,
    }
}

fn get_column<S: StorageSchema>(db: &DB) -> Result<&ColumnFamily, RocksAdapterError> {
    let category = map_category(S::category());

    let column = db
        .cf_handle(category)
        .ok_or_else(|| RocksAdapterError::from(category))?;

    Ok(column)
}
