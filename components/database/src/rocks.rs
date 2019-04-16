use std::sync::Arc;

use rocksdb::{ColumnFamily, Error as RocksError, Options, WriteBatch, DB};
use tokio_async_await::compat::backward::Compat;

use core_runtime::{DataCategory, Database, DatabaseError, FutDBResult};

pub struct RocksDB {
    db: Arc<DB>,
}

impl RocksDB {
    // TODO: Configure RocksDB options for each column.
    pub fn new(path: &str) -> Result<Self, DatabaseError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let categories = [
            map_data_category(DataCategory::Block),
            map_data_category(DataCategory::Transaction),
            map_data_category(DataCategory::Receipt),
            map_data_category(DataCategory::State),
            map_data_category(DataCategory::TransactionPool),
            map_data_category(DataCategory::TransactionPosition),
        ];
        let db = DB::open_cf(&opts, path, categories.iter())
            .map_err(|e| DatabaseError::Internal(e.to_string()))?;

        Ok(RocksDB { db: Arc::new(db) })
    }

    #[cfg(test)]
    pub fn clean(&self) {
        let categories = [
            map_data_category(DataCategory::Block),
            map_data_category(DataCategory::Transaction),
            map_data_category(DataCategory::Receipt),
            map_data_category(DataCategory::State),
            map_data_category(DataCategory::TransactionPool),
            map_data_category(DataCategory::TransactionPosition),
        ];

        for c in categories.iter() {
            self.db.drop_cf(c).unwrap();
        }
    }
}

impl Database for RocksDB {
    fn get(&self, c: DataCategory, key: &[u8]) -> FutDBResult<Option<Vec<u8>>> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            let v = db.get_cf(column, &key).map_err(map_db_err)?;
            Ok(v.map(|v| v.to_vec()))
        };
        Box::new(Compat::new(fut))
    }

    fn get_batch(&self, c: DataCategory, keys: &[Vec<u8>]) -> FutDBResult<Vec<Option<Vec<u8>>>> {
        let db = Arc::clone(&self.db);
        let keys = keys.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            let mut values = Vec::with_capacity(keys.len());

            for key in keys {
                let v = db.get_cf(column, key).map_err(map_db_err)?;
                values.push(v.map(|v| v.to_vec()));
            }
            Ok(values)
        };
        Box::new(Compat::new(fut))
    }

    fn insert(&self, c: DataCategory, key: Vec<u8>, value: Vec<u8>) -> FutDBResult<()> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            let column = get_column(&db, c)?;
            db.put_cf(column, key, value).map_err(map_db_err)?;
            Ok(())
        };
        Box::new(Compat::new(fut))
    }

    fn insert_batch(
        &self,
        c: DataCategory,
        keys: Vec<Vec<u8>>,
        values: Vec<Vec<u8>>,
    ) -> FutDBResult<()> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            if keys.len() != values.len() {
                return Err(DatabaseError::InvalidData);
            }

            let column = get_column(&db, c)?;
            let mut batch = WriteBatch::default();

            for i in 0..keys.len() {
                batch
                    .put_cf(column, &keys[i], &values[i])
                    .map_err(map_db_err)?;
            }
            db.write(batch).map_err(map_db_err)?;
            Ok(())
        };
        Box::new(Compat::new(fut))
    }

    fn contains(&self, c: DataCategory, key: &[u8]) -> FutDBResult<bool> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            let v = db.get_cf(column, &key).map_err(map_db_err)?;
            Ok(v.is_some())
        };
        Box::new(Compat::new(fut))
    }

    fn remove(&self, c: DataCategory, key: &[u8]) -> FutDBResult<()> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            db.delete_cf(column, key).map_err(map_db_err)?;
            Ok(())
        };
        Box::new(Compat::new(fut))
    }

    fn remove_batch(&self, c: DataCategory, keys: &[Vec<u8>]) -> FutDBResult<()> {
        let db = Arc::clone(&self.db);
        let keys = keys.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;

            let mut batch = WriteBatch::default();
            for key in keys {
                batch.delete_cf(column, key).map_err(map_db_err)?;
            }
            db.write(batch).map_err(map_db_err)?;
            Ok(())
        };
        Box::new(Compat::new(fut))
    }
}

const C_BLOCK: &str = "c1";
const C_TRANSACTION: &str = "c2";
const C_RECEIPT: &str = "c3";
const C_STATE: &str = "c4";
const C_TRANSACTION_POOL: &str = "c5";
const C_TRANSACTION_POSITION: &str = "c6";

fn map_data_category(category: DataCategory) -> &'static str {
    match category {
        DataCategory::Block => C_BLOCK,
        DataCategory::Transaction => C_TRANSACTION,
        DataCategory::Receipt => C_RECEIPT,
        DataCategory::State => C_STATE,
        DataCategory::TransactionPool => C_TRANSACTION_POOL,
        DataCategory::TransactionPosition => C_TRANSACTION_POSITION,
    }
}

fn map_db_err(err: RocksError) -> DatabaseError {
    DatabaseError::Internal(err.to_string())
}

fn get_column(db: &DB, c: DataCategory) -> Result<ColumnFamily, DatabaseError> {
    db.cf_handle(map_data_category(c))
        .ok_or(DatabaseError::NotFound)
}

// TODO: merge rocksdb and memorydb test together.
#[cfg(test)]
mod tests {
    use futures::future::Future;

    use core_runtime::{DataCategory, Database};

    use super::RocksDB;

    #[test]
    fn test_get_should_return_ok() {
        let db = RocksDB::new("rocksdb/test_get_should_return_ok").unwrap();

        assert_eq!(db.get(DataCategory::Block, b"test").wait(), Ok(None));
        db.insert(DataCategory::Block, b"test".to_vec(), b"test".to_vec())
            .wait()
            .unwrap();
        let v = db.get(DataCategory::Block, b"test").wait().unwrap();
        assert_eq!(v, Some(b"test".to_vec()));
        db.clean();
    }

    #[test]
    fn test_insert_should_return_ok() {
        let db = RocksDB::new("rocksdb/test_insert_should_return_ok").unwrap();

        db.insert(DataCategory::Block, b"test".to_vec(), b"test".to_vec())
            .wait()
            .unwrap();
        assert_eq!(
            Some(b"test".to_vec()),
            db.get(DataCategory::Block, b"test").wait().unwrap()
        );
        db.clean();
    }

    #[test]
    fn test_insert_batch_should_return_ok() {
        let db = RocksDB::new("rocksdb/test_insert_batch_should_return_ok").unwrap();

        db.insert_batch(
            DataCategory::Block,
            vec![b"test1".to_vec(), b"test2".to_vec()],
            vec![b"test1".to_vec(), b"test2".to_vec()],
        )
        .wait()
        .unwrap();
        assert_eq!(
            Some(b"test1".to_vec()),
            db.get(DataCategory::Block, b"test1").wait().unwrap()
        );
        assert_eq!(
            Some(b"test2".to_vec()),
            db.get(DataCategory::Block, b"test2").wait().unwrap()
        );
        db.clean();
    }

    #[test]
    fn test_contain_should_return_true() {
        let db = RocksDB::new("rocksdb/test_contain_should_return_true").unwrap();

        db.insert(DataCategory::Block, b"test".to_vec(), b"test".to_vec())
            .wait()
            .unwrap();
        assert_eq!(
            db.contains(DataCategory::Block, b"test").wait().unwrap(),
            true
        );

        db.clean()
    }

    #[test]
    fn test_contain_should_return_false() {
        let db = RocksDB::new("rocksdb/test_contain_should_return_false").unwrap();

        assert_eq!(
            db.contains(DataCategory::Block, b"test").wait().unwrap(),
            false
        );
        db.clean();
    }

    #[test]
    fn test_remove_should_return_ok() {
        let db = RocksDB::new("rocksdb/test_remove_should_return_ok").unwrap();

        db.insert(DataCategory::Block, b"test".to_vec(), b"test".to_vec())
            .wait()
            .unwrap();
        db.remove(DataCategory::Block, b"test").wait().unwrap();
        assert_eq!(db.get(DataCategory::Block, b"test").wait(), Ok(None));
        db.clean();
    }

    #[test]
    fn test_remove_batch_should_return_ok() {
        let db = RocksDB::new("rocksdb/test_remove_batch_should_return_ok").unwrap();

        db.insert_batch(
            DataCategory::Block,
            vec![b"test1".to_vec(), b"test2".to_vec()],
            vec![b"test1".to_vec(), b"test2".to_vec()],
        )
        .wait()
        .unwrap();
        db.remove_batch(DataCategory::Block, &[b"test1".to_vec(), b"test2".to_vec()])
            .wait()
            .unwrap();
        assert_eq!(db.get(DataCategory::Block, b"test1").wait(), Ok(None));
        assert_eq!(db.get(DataCategory::Block, b"test2").wait(), Ok(None));
        db.clean();
    }
}
