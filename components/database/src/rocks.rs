use std::sync::Arc;

use futures::future::{err, ok, result, Future};
use rocksdb::{ColumnFamily, Error as RocksError, Options, WriteBatch, DB};

use core_runtime::{
    DataCategory, DatabaseError, DatabaseFactory, DatabaseInstance, FutRuntimeResult,
};

pub struct Factory {
    db: Arc<DB>,
}

impl Factory {
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
        ];
        let db = DB::open_cf(&opts, path, categories.iter())
            .map_err(|e| DatabaseError::Internal(e.to_string()))?;

        Ok(Factory { db: Arc::new(db) })
    }

    #[cfg(test)]
    pub fn clean(&self) {
        let categories = [
            map_data_category(DataCategory::Block),
            map_data_category(DataCategory::Transaction),
            map_data_category(DataCategory::Receipt),
            map_data_category(DataCategory::State),
            map_data_category(DataCategory::TransactionPool),
        ];

        for c in categories.iter() {
            self.db.drop_cf(c).unwrap();
        }
    }
}

impl DatabaseFactory for Factory {
    type Instance = Instance;

    fn crate_instance(&self) -> FutRuntimeResult<Self::Instance, DatabaseError> {
        Box::new(ok(Instance {
            db: Arc::clone(&self.db),
        }))
    }
}

pub struct Instance {
    db: Arc<DB>,
}

impl Instance {
    fn get_column(&self, c: DataCategory) -> Result<ColumnFamily, DatabaseError> {
        self.db
            .cf_handle(map_data_category(c))
            .ok_or(DatabaseError::NotFound)
    }
}

impl DatabaseInstance for Instance {
    fn get(&self, c: DataCategory, key: &[u8]) -> FutRuntimeResult<Vec<u8>, DatabaseError> {
        let column = match self.get_column(c) {
            Ok(column) => column,
            Err(e) => return Box::new(err(e)),
        };

        let v = match self.db.get_cf(column, key) {
            Ok(opt_v) => match opt_v {
                Some(v) => v.to_vec(),
                None => return Box::new(err(DatabaseError::NotFound)),
            },
            Err(e) => return Box::new(err(map_db_err(e))),
        };

        Box::new(ok(v.to_vec()))
    }

    fn get_batch(
        &self,
        c: DataCategory,
        keys: &[Vec<u8>],
    ) -> FutRuntimeResult<Vec<Option<Vec<u8>>>, DatabaseError> {
        let column = match self.get_column(c) {
            Ok(column) => column,
            Err(e) => return Box::new(err(e)),
        };

        let values: Vec<Option<Vec<u8>>> = keys
            .iter()
            .map(|key| match self.db.get_cf(column, key) {
                Ok(opt_v) => match opt_v {
                    Some(v) => Some(v.to_vec()),
                    None => None,
                },
                Err(e) => {
                    log::error!(target: "database rocksdb", "{}", e);
                    None
                }
            })
            .collect();

        Box::new(ok(values))
    }

    fn insert(
        &mut self,
        c: DataCategory,
        key: &[u8],
        value: &[u8],
    ) -> FutRuntimeResult<(), DatabaseError> {
        let column = match self.get_column(c) {
            Ok(column) => column,
            Err(e) => return Box::new(err(e)),
        };

        let fut = result(self.db.put_cf(column, key, value)).map_err(map_db_err);
        Box::new(fut)
    }

    fn insert_batch(
        &mut self,
        c: DataCategory,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
    ) -> FutRuntimeResult<(), DatabaseError> {
        if keys.len() != values.len() {
            return Box::new(err(DatabaseError::InvalidData));
        }

        let column = match self.get_column(c) {
            Ok(column) => column,
            Err(e) => return Box::new(err(e)),
        };

        let mut batch = WriteBatch::default();
        for i in 0..keys.len() {
            if let Err(e) = batch.put_cf(column, &keys[i], &values[i]) {
                return Box::new(err(map_db_err(e)));
            }
        }

        let fut = result(self.db.write(batch)).map_err(map_db_err);
        Box::new(fut)
    }

    fn contains(&self, c: DataCategory, key: &[u8]) -> FutRuntimeResult<bool, DatabaseError> {
        let column = match self.get_column(c) {
            Ok(column) => column,
            Err(e) => return Box::new(err(e)),
        };

        let v = match self.db.get_cf(column, key) {
            Ok(opt_v) => opt_v.is_some(),
            Err(e) => return Box::new(err(map_db_err(e))),
        };

        Box::new(ok(v))
    }

    fn remove(&mut self, c: DataCategory, key: &[u8]) -> FutRuntimeResult<(), DatabaseError> {
        let column = match self.get_column(c) {
            Ok(column) => column,
            Err(e) => return Box::new(err(e)),
        };

        let fut = result(self.db.delete_cf(column, key)).map_err(map_db_err);
        Box::new(fut)
    }

    fn remove_batch(
        &mut self,
        c: DataCategory,
        keys: &[Vec<u8>],
    ) -> FutRuntimeResult<(), DatabaseError> {
        let column = match self.get_column(c) {
            Ok(column) => column,
            Err(e) => return Box::new(err(e)),
        };

        let mut batch = WriteBatch::default();
        for key in keys {
            if let Err(e) = batch.delete_cf(column, &key) {
                return Box::new(err(map_db_err(e)));
            }
        }
        let fut = result(self.db.write(batch)).map_err(map_db_err);
        Box::new(fut)
    }
}

const C_BLOCK: &str = "c1";
const C_TRANSACTION: &str = "c2";
const C_RECEIPT: &str = "c3";
const C_STATE: &str = "c4";
const C_TRANSACTION_POOL: &str = "c5";

fn map_data_category(category: DataCategory) -> &'static str {
    match category {
        DataCategory::Block => C_BLOCK,
        DataCategory::Transaction => C_TRANSACTION,
        DataCategory::Receipt => C_RECEIPT,
        DataCategory::State => C_STATE,
        DataCategory::TransactionPool => C_TRANSACTION_POOL,
    }
}

fn map_db_err(err: RocksError) -> DatabaseError {
    DatabaseError::Internal(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::Factory;

    use core_runtime::{DataCategory, DatabaseError, DatabaseFactory, DatabaseInstance};
    use futures::future::Future;

    #[test]
    fn test_get_should_return_ok() {
        let f = Factory::new("rocksdb/test_get_should_return_ok").unwrap();
        let mut instance = f.crate_instance().wait().unwrap();

        check_not_found(instance.get(DataCategory::Block, b"test").wait());
        instance
            .insert(DataCategory::Block, b"test", b"test")
            .wait()
            .unwrap();
        let v = instance.get(DataCategory::Block, b"test").wait().unwrap();
        assert_eq!(v, b"test".to_vec());
        f.clean();
    }

    #[test]
    fn test_insert_should_return_ok() {
        let f = Factory::new("rocksdb/test_insert_should_return_ok").unwrap();
        let mut instance = f.crate_instance().wait().unwrap();

        instance
            .insert(DataCategory::Block, b"test", b"test")
            .wait()
            .unwrap();
        assert_eq!(
            b"test".to_vec(),
            instance.get(DataCategory::Block, b"test").wait().unwrap()
        );
        f.clean();
    }

    #[test]
    fn test_insert_batch_should_return_ok() {
        let f = Factory::new("rocksdb/test_insert_batch_should_return_ok").unwrap();
        let mut instance = f.crate_instance().wait().unwrap();

        instance
            .insert_batch(
                DataCategory::Block,
                &[b"test1".to_vec(), b"test2".to_vec()],
                &[b"test1".to_vec(), b"test2".to_vec()],
            )
            .wait()
            .unwrap();
        assert_eq!(
            b"test1".to_vec(),
            instance.get(DataCategory::Block, b"test1").wait().unwrap()
        );
        assert_eq!(
            b"test2".to_vec(),
            instance.get(DataCategory::Block, b"test2").wait().unwrap()
        );
        f.clean();
    }

    #[test]
    fn test_contain_should_return_true() {
        let f = Factory::new("rocksdb/test_contain_should_return_true").unwrap();
        let mut instance = f.crate_instance().wait().unwrap();

        instance
            .insert(DataCategory::Block, b"test", b"test")
            .wait()
            .unwrap();
        assert_eq!(
            instance
                .contains(DataCategory::Block, b"test")
                .wait()
                .unwrap(),
            true
        );

        f.clean()
    }

    #[test]
    fn test_contain_should_return_false() {
        let f = Factory::new("rocksdb/test_contain_should_return_false").unwrap();
        let instance = f.crate_instance().wait().unwrap();

        assert_eq!(
            instance
                .contains(DataCategory::Block, b"test")
                .wait()
                .unwrap(),
            false
        );
        f.clean();
    }

    #[test]
    fn test_remove_should_return_ok() {
        let f = Factory::new("rocksdb/test_remove_should_return_ok").unwrap();
        let mut instance = f.crate_instance().wait().unwrap();

        instance
            .insert(DataCategory::Block, b"test", b"test")
            .wait()
            .unwrap();
        instance
            .remove(DataCategory::Block, b"test")
            .wait()
            .unwrap();
        check_not_found(instance.get(DataCategory::Block, b"test").wait());
        f.clean();
    }

    #[test]
    fn test_remove_batch_should_return_ok() {
        let f = Factory::new("rocksdb/test_remove_batch_should_return_ok").unwrap();
        let mut instance = f.crate_instance().wait().unwrap();

        instance
            .insert_batch(
                DataCategory::Block,
                &[b"test1".to_vec(), b"test2".to_vec()],
                &[b"test1".to_vec(), b"test2".to_vec()],
            )
            .wait()
            .unwrap();
        instance
            .remove_batch(DataCategory::Block, &[b"test1".to_vec(), b"test2".to_vec()])
            .wait()
            .unwrap();
        check_not_found(instance.get(DataCategory::Block, b"test1").wait());
        check_not_found(instance.get(DataCategory::Block, b"test2").wait());
        f.clean();
    }

    fn check_not_found<T>(res: Result<T, DatabaseError>) {
        match res {
            Ok(_) => panic!("The result must be an error not found"),
            Err(e) => assert_eq!(e, DatabaseError::NotFound),
        }
    }
}
