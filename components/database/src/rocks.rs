use std::default::Default;
use std::path::Path;
use std::sync::Arc;

use rocksdb::{BlockBasedOptions, ColumnFamily, Error as RocksError, Options, WriteBatch, DB};

use core_context::Context;
use core_runtime::{DataCategory, Database, DatabaseError, FutDBResult};

pub struct RocksDB {
    db: Arc<DB>,
}

#[derive(Debug)]
pub struct Config {
    pub block_size:                     Option<usize>,
    pub block_cache_size:               Option<u64>,
    pub max_bytes_for_level_base:       Option<u64>,
    pub max_bytes_for_level_multiplier: Option<f64>,
    pub write_buffer_size:              Option<usize>,
    pub target_file_size_base:          Option<u64>,
    pub max_write_buffer_number:        Option<i32>,
    pub max_background_compactions:     Option<i32>,
    pub max_background_flushes:         Option<i32>,
    pub increase_parallelism:           Option<i32>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            block_size:                     None,
            block_cache_size:               None,
            max_bytes_for_level_base:       None,
            max_bytes_for_level_multiplier: None,
            write_buffer_size:              None,
            target_file_size_base:          None,
            max_write_buffer_number:        None,
            max_background_compactions:     None,
            max_background_flushes:         None,
            increase_parallelism:           None,
        }
    }
}

impl RocksDB {
    // TODO: Configure RocksDB options for each column.
    pub fn new<P: AsRef<Path>>(path: P, conf: &Config) -> Result<Self, DatabaseError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        if let Some(size) = conf.block_cache_size {
            opts.optimize_for_point_lookup(size);
        }

        if let Some(level_base) = conf.max_bytes_for_level_base {
            opts.set_max_bytes_for_level_base(level_base);
        }

        if let Some(level_multiplier) = conf.max_bytes_for_level_multiplier {
            opts.set_max_bytes_for_level_multiplier(level_multiplier);
        }

        if let Some(size) = conf.write_buffer_size {
            opts.set_write_buffer_size(size);
        }

        if let Some(size_base) = conf.target_file_size_base {
            opts.set_target_file_size_base(size_base);
        }

        if let Some(number) = conf.max_write_buffer_number {
            opts.set_max_write_buffer_number(number);
        }

        if let Some(compactions) = conf.max_background_compactions {
            opts.set_max_background_compactions(compactions);
        }

        if let Some(flushes) = conf.max_background_flushes {
            opts.set_max_background_flushes(flushes);
        }

        if let Some(parallelism) = conf.increase_parallelism {
            opts.increase_parallelism(parallelism);
        }

        let mut block_opts = BlockBasedOptions::default();

        if let Some(block_size) = conf.block_size {
            block_opts.set_block_size(block_size);
        }
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
    fn clean(&self) {
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

        Box::pin(fut)
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

        Box::pin(fut)
    }

    fn insert(&self, _: Context, c: DataCategory, key: Vec<u8>, value: Vec<u8>) -> FutDBResult<()> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            let column = get_column(&db, c)?;
            db.put_cf(column, key, value).map_err(map_db_err)?;
            Ok(())
        };

        Box::pin(fut)
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

        Box::pin(fut)
    }

    fn contains(&self, _: Context, c: DataCategory, key: &[u8]) -> FutDBResult<bool> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            let v = db.get_cf(column, &key).map_err(map_db_err)?;
            Ok(v.is_some())
        };

        Box::pin(fut)
    }

    fn remove(&self, _: Context, c: DataCategory, key: &[u8]) -> FutDBResult<()> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let fut = async move {
            let column = get_column(&db, c)?;
            db.delete_cf(column, key).map_err(map_db_err)?;
            Ok(())
        };

        Box::pin(fut)
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

        Box::pin(fut)
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
    use crate::test::{
        test_contains, test_get, test_insert, test_insert_batch, test_remove, test_remove_batch,
    };

    use super::{Config, RocksDB};

    #[test]
    fn test_get_should_pass() {
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_get_should_return_ok", &cfg).unwrap();

        test_get(&db);
        db.clean();
    }

    #[test]
    fn test_insert_should_pass() {
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_insert_should_return_ok", &cfg).unwrap();

        test_insert(&db);
        db.clean();
    }

    #[test]
    fn test_insert_batch_should_pass() {
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_insert_batch_should_return_ok", &cfg).unwrap();

        test_insert_batch(&db);
        db.clean();
    }

    #[test]
    fn test_contain_should_pass() {
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_contain_should_return_true", &cfg).unwrap();

        test_contains(&db);
        db.clean()
    }

    #[test]
    fn test_remove_should_pass() {
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_remove_should_return_ok", &cfg).unwrap();

        test_remove(&db);
        db.clean();
    }

    #[test]
    fn test_remove_batch_should_pass() {
        let cfg = Config::default();
        let db = RocksDB::new("rocksdb/test_remove_batch_should_return_ok", &cfg).unwrap();

        test_remove_batch(&db);
        db.clean();
    }
}
