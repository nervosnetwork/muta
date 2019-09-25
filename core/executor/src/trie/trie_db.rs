use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use derive_more::{Display, From};
use rocksdb::{Options, WriteBatch, DB};

use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub struct RocksTrieDB {
    light: bool,
    db:    Arc<DB>,
}

impl RocksTrieDB {
    pub fn new<P: AsRef<Path>>(path: P, light: bool) -> ProtocolResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let db = DB::open(&opts, path).map_err(RocksTrieDBError::from)?;

        Ok(RocksTrieDB {
            light,
            db: Arc::new(db),
        })
    }
}

impl cita_trie::DB for RocksTrieDB {
    type Error = RocksTrieDBError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self
            .db
            .get(key)
            .map_err(RocksTrieDBError::from)?
            .map(|v| v.to_vec()))
    }

    fn contains(&self, key: &[u8]) -> Result<bool, Self::Error> {
        Ok(self.db.get(key).map_err(RocksTrieDBError::from)?.is_some())
    }

    fn insert(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Self::Error> {
        self.db
            .put(Bytes::from(key), Bytes::from(value))
            .map_err(RocksTrieDBError::from)?;
        Ok(())
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        if keys.len() != values.len() {
            return Err(RocksTrieDBError::BatchLengthMismatch);
        }

        let mut batch = WriteBatch::default();
        for i in 0..keys.len() {
            let key = &keys[i];
            let value = &values[i];
            batch.put(key, value).map_err(RocksTrieDBError::from)?;
        }

        self.db.write(batch).map_err(RocksTrieDBError::from)?;
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        if self.light {
            self.db.delete(key).map_err(RocksTrieDBError::from)?;
        }
        Ok(())
    }

    fn remove_batch(&self, keys: &[Vec<u8>]) -> Result<(), Self::Error> {
        if self.light {
            let mut batch = WriteBatch::default();
            for key in keys {
                batch.delete(key).map_err(RocksTrieDBError::from)?;
            }

            self.db.write(batch).map_err(RocksTrieDBError::from)?;
        }

        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum RocksTrieDBError {
    #[display(fmt = "rocksdb {}", _0)]
    RocksDB(rocksdb::Error),

    #[display(fmt = "parameters do not match")]
    InsertParameter,

    #[display(fmt = "batch length dont match")]
    BatchLengthMismatch,
}

impl Error for RocksTrieDBError {}

impl From<RocksTrieDBError> for ProtocolError {
    fn from(err: RocksTrieDBError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Executor, Box::new(err))
    }
}
