use std::default::Default;
use std::path::Path;
use std::sync::Arc;

use futures::prelude::{FutureExt, TryFutureExt};
use rocksdb::{BlockBasedOptions, ColumnFamily, Error as RocksError, Options, WriteBatch, DB};

use core_context::Context;
use core_runtime::{DataCategory, Database, DatabaseError, FutDBResult};

pub struct RocksDB {
    db: Arc<DB>,
}

#[derive(Debug)]
pub struct Config {
    pub block_size:                     usize,
    pub block_cache_size:               u64,
    pub max_bytes_for_level_base:       u64,
    pub max_bytes_for_level_multiplier: f64,
    pub write_buffer_size:              usize,
    pub target_file_size_base:          u64,
    pub max_write_buffer_number:        i32,
    pub max_background_compactions:     i32,
    pub max_background_flushes:         i32,
    pub increase_parallelism:           i32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            block_size:                     8192,              // 8KB
            block_cache_size:               1024 * 1024 * 8,   // 8 MiB
            max_bytes_for_level_base:       1024 * 1024 * 256, // 256 MiB
            max_bytes_for_level_multiplier: 10.0,
            write_buffer_size:              1024 * 1024 * 64, // 64 MiB
            target_file_size_base:          1024 * 1024 * 64, // 64 MiB
            max_write_buffer_number:        4,
            max_background_compactions:     2,
            max_background_flushes:         2,
            increase_parallelism:           3,
        }
    }
}

impl RocksDB {
    // TODO: Configure RocksDB options for each column.
    pub fn new<P: AsRef<Path>>(path: P, conf: &Config) -> Result<Self, DatabaseError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        opts.optimize_for_point_lookup(conf.block_cache_size);
        opts.set_max_bytes_for_level_base(conf.max_bytes_for_level_base);
        opts.set_max_bytes_for_level_multiplier(conf.max_bytes_for_level_multiplier);
        opts.set_write_buffer_size(conf.write_buffer_size);
        opts.set_target_file_size_base(conf.target_file_size_base);
        opts.set_max_write_buffer_number(conf.max_write_buffer_number);
        opts.set_max_background_compactions(conf.max_background_compactions);
        opts.set_max_background_flushes(conf.max_background_flushes);
        opts.increase_parallelism(conf.increase_parallelism);

        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_size(conf.block_size);

        opts.set_block_based_table_factory(&block_opts);

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
    fn get(&self, _: Context, c: DataCategory, key: &[u8]) -> FutDBResult<Option<Vec<u8>>> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            let v = db.get_cf(column, &key).map_err(map_db_err)?;
            Ok(v.map(|v| v.to_vec()))
        };
        Box::new(fut.boxed().compat())
    }

    fn get_batch(
        &self,
        _: Context,
        c: DataCategory,
        keys: &[Vec<u8>],
    ) -> FutDBResult<Vec<Option<Vec<u8>>>> {
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
        Box::new(fut.boxed().compat())
    }

    fn insert(&self, _: Context, c: DataCategory, key: Vec<u8>, value: Vec<u8>) -> FutDBResult<()> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            let column = get_column(&db, c)?;
            db.put_cf(column, key, value).map_err(map_db_err)?;
            Ok(())
        };
        Box::new(fut.boxed().compat())
    }

    fn insert_batch(
        &self,
        _: Context,
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
        Box::new(fut.boxed().compat())
    }

    fn contains(&self, _: Context, c: DataCategory, key: &[u8]) -> FutDBResult<bool> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            let v = db.get_cf(column, &key).map_err(map_db_err)?;
            Ok(v.is_some())
        };
        Box::new(fut.boxed().compat())
    }

    fn remove(&self, _: Context, c: DataCategory, key: &[u8]) -> FutDBResult<()> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            db.delete_cf(column, key).map_err(map_db_err)?;
            Ok(())
        };
        Box::new(fut.boxed().compat())
    }

    fn remove_batch(&self, _: Context, c: DataCategory, keys: &[Vec<u8>]) -> FutDBResult<()> {
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
        Box::new(fut.boxed().compat())
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
    use futures01::future::Future;

    use core_context::Context;
    use core_runtime::{DataCategory, Database};

    use super::{Config, RocksDB};

    #[test]
    fn test_get_should_return_ok() {
        let ctx = Context::new();
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_get_should_return_ok", &cfg).unwrap();

        assert_eq!(
            db.get(ctx.clone(), DataCategory::Block, b"test").wait(),
            Ok(None)
        );
        db.insert(
            ctx.clone(),
            DataCategory::Block,
            b"test".to_vec(),
            b"test".to_vec(),
        )
        .wait()
        .unwrap();
        let v = db.get(ctx, DataCategory::Block, b"test").wait().unwrap();
        assert_eq!(v, Some(b"test".to_vec()));
        db.clean();
    }

    #[test]
    fn test_insert_should_return_ok() {
        let ctx = Context::new();
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_insert_should_return_ok", &cfg).unwrap();

        db.insert(
            ctx.clone(),
            DataCategory::Block,
            b"test".to_vec(),
            b"test".to_vec(),
        )
        .wait()
        .unwrap();
        assert_eq!(
            Some(b"test".to_vec()),
            db.get(ctx.clone(), DataCategory::Block, b"test")
                .wait()
                .unwrap()
        );
        db.clean();
    }

    #[test]
    fn test_insert_batch_should_return_ok() {
        let ctx = Context::new();
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_insert_batch_should_return_ok", &cfg).unwrap();

        db.insert_batch(
            ctx.clone(),
            DataCategory::Block,
            vec![b"test1".to_vec(), b"test2".to_vec()],
            vec![b"test1".to_vec(), b"test2".to_vec()],
        )
        .wait()
        .unwrap();
        assert_eq!(
            Some(b"test1".to_vec()),
            db.get(ctx.clone(), DataCategory::Block, b"test1")
                .wait()
                .unwrap()
        );
        assert_eq!(
            Some(b"test2".to_vec()),
            db.get(ctx, DataCategory::Block, b"test2").wait().unwrap()
        );
        db.clean();
    }

    #[test]
    fn test_contain_should_return_true() {
        let ctx = Context::new();
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_contain_should_return_true", &cfg).unwrap();

        db.insert(
            ctx.clone(),
            DataCategory::Block,
            b"test".to_vec(),
            b"test".to_vec(),
        )
        .wait()
        .unwrap();
        assert_eq!(
            db.contains(ctx.clone(), DataCategory::Block, b"test")
                .wait()
                .unwrap(),
            true
        );

        db.clean()
    }

    #[test]
    fn test_contain_should_return_false() {
        let ctx = Context::new();
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_contain_should_return_false", &cfg).unwrap();

        assert_eq!(
            db.contains(ctx.clone(), DataCategory::Block, b"test")
                .wait()
                .unwrap(),
            false
        );
        db.clean();
    }

    #[test]
    fn test_remove_should_return_ok() {
        let ctx = Context::new();
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_remove_should_return_ok", &cfg).unwrap();

        db.insert(
            ctx.clone(),
            DataCategory::Block,
            b"test".to_vec(),
            b"test".to_vec(),
        )
        .wait()
        .unwrap();
        db.remove(ctx.clone(), DataCategory::Block, b"test")
            .wait()
            .unwrap();
        assert_eq!(db.get(ctx, DataCategory::Block, b"test").wait(), Ok(None));
        db.clean();
    }

    #[test]
    fn test_remove_batch_should_return_ok() {
        let ctx = Context::new();
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_remove_batch_should_return_ok", &cfg).unwrap();

        db.insert_batch(
            ctx.clone(),
            DataCategory::Block,
            vec![b"test1".to_vec(), b"test2".to_vec()],
            vec![b"test1".to_vec(), b"test2".to_vec()],
        )
        .wait()
        .unwrap();
        db.remove_batch(ctx.clone(), DataCategory::Block, &[
            b"test1".to_vec(),
            b"test2".to_vec(),
        ])
        .wait()
        .unwrap();
        assert_eq!(
            db.get(ctx.clone(), DataCategory::Block, b"test1").wait(),
            Ok(None)
        );
        assert_eq!(db.get(ctx, DataCategory::Block, b"test2").wait(), Ok(None));
        db.clean();
    }
}
