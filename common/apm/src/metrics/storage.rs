use std::time::Duration;

use lazy_static::lazy_static;
use protocol::traits::StorageCategory;

use crate::metrics::{
    auto_flush_from, duration_to_sec, make_auto_flush_static_metric, register_counter_vec,
    register_int_counter_vec, CounterVec, IntCounterVec,
};

make_auto_flush_static_metric! {
  pub label_enum COLUMN_FAMILY_TYPES {
    block,
    block_header,
    receipt,
    signed_tx,
    wal,
    hash_height,
    state,
  }

  pub struct StoragePutCfTimeUsageVec: LocalCounter {
    "cf" => COLUMN_FAMILY_TYPES
  }

  pub struct StoragePutCfBytesVec: LocalIntCounter {
    "cf" => COLUMN_FAMILY_TYPES
  }

  pub struct StorageGetCfTimeUsageVec: LocalCounter {
    "cf" => COLUMN_FAMILY_TYPES
  }

  pub struct StorageGetCfTotalVec: LocalIntCounter {
    "cf" => COLUMN_FAMILY_TYPES
  }
}

lazy_static! {
    pub static ref STORAGE_PUT_CF_TIME_USAGE_VEC: CounterVec = register_counter_vec!(
        "muta_storage_put_cf_seconds",
        "Storage put_cf time usage",
        &["cf"]
    )
    .unwrap();
    pub static ref STORAGE_PUT_CF_BYTES_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_storage_put_cf_bytes",
        "Storage total insert bytes",
        &["cf"]
    )
    .unwrap();
    pub static ref STORAGE_GET_CF_TIME_USAGE_VEC: CounterVec = register_counter_vec!(
        "muta_storage_get_cf_seconds",
        "Storage get_cf time usage",
        &["cf"]
    )
    .unwrap();
    pub static ref STORAGE_GET_CF_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_storage_get_cf_total",
        "Storage total get_cf keys number",
        &["cf"]
    )
    .unwrap();
}

lazy_static! {
    pub static ref STORAGE_PUT_CF_TIME_USAGE: StoragePutCfTimeUsageVec =
        auto_flush_from!(STORAGE_PUT_CF_TIME_USAGE_VEC, StoragePutCfTimeUsageVec);
    pub static ref STORAGE_PUT_CF_BYTES_COUNTER: StoragePutCfBytesVec =
        auto_flush_from!(STORAGE_PUT_CF_BYTES_COUNTER_VEC, StoragePutCfBytesVec);
    pub static ref STORAGE_GET_CF_TIME_USAGE: StorageGetCfTimeUsageVec =
        auto_flush_from!(STORAGE_GET_CF_TIME_USAGE_VEC, StorageGetCfTimeUsageVec);
    pub static ref STORAGE_GET_CF_COUNTER: StorageGetCfTotalVec =
        auto_flush_from!(STORAGE_GET_CF_COUNTER_VEC, StorageGetCfTotalVec);
}

pub fn on_storage_get_state(duration: Duration, keys: i64) {
    let seconds = duration_to_sec(duration);

    STORAGE_GET_CF_TIME_USAGE.state.inc_by(seconds);
    STORAGE_GET_CF_COUNTER.state.inc_by(keys);
}

pub fn on_storage_put_state(duration: Duration, size: i64) {
    let seconds = duration_to_sec(duration);

    STORAGE_PUT_CF_TIME_USAGE.state.inc_by(seconds);
    STORAGE_PUT_CF_BYTES_COUNTER.state.inc_by(size);
}

pub fn on_storage_get_cf(sc: StorageCategory, duration: Duration, keys: i64) {
    let seconds = duration_to_sec(duration);

    match sc {
        StorageCategory::Block => {
            STORAGE_GET_CF_TIME_USAGE.block.inc_by(seconds);
            STORAGE_GET_CF_COUNTER.block.inc_by(keys);
        }
        StorageCategory::BlockHeader => {
            STORAGE_GET_CF_TIME_USAGE.block_header.inc_by(seconds);
            STORAGE_GET_CF_COUNTER.block_header.inc_by(keys);
        }
        StorageCategory::Receipt => {
            STORAGE_GET_CF_TIME_USAGE.receipt.inc_by(seconds);
            STORAGE_GET_CF_COUNTER.receipt.inc_by(keys);
        }
        StorageCategory::Wal => {
            STORAGE_GET_CF_TIME_USAGE.wal.inc_by(seconds);
            STORAGE_GET_CF_COUNTER.wal.inc_by(keys);
        }
        StorageCategory::SignedTransaction => {
            STORAGE_GET_CF_TIME_USAGE.signed_tx.inc_by(seconds);
            STORAGE_GET_CF_COUNTER.signed_tx.inc_by(keys);
        }
        StorageCategory::HashHeight => {
            STORAGE_GET_CF_TIME_USAGE.hash_height.inc_by(seconds);
            STORAGE_GET_CF_COUNTER.hash_height.inc_by(keys);
        }
    }
}

pub fn on_storage_put_cf(sc: StorageCategory, duration: Duration, size: i64) {
    let seconds = duration_to_sec(duration);

    match sc {
        StorageCategory::Block => {
            STORAGE_PUT_CF_TIME_USAGE.block.inc_by(seconds);
            STORAGE_PUT_CF_BYTES_COUNTER.block.inc_by(size);
        }
        StorageCategory::BlockHeader => {
            STORAGE_PUT_CF_TIME_USAGE.block_header.inc_by(seconds);
            STORAGE_PUT_CF_BYTES_COUNTER.block_header.inc_by(size);
        }
        StorageCategory::Receipt => {
            STORAGE_PUT_CF_TIME_USAGE.receipt.inc_by(seconds);
            STORAGE_PUT_CF_BYTES_COUNTER.receipt.inc_by(size);
        }
        StorageCategory::Wal => {
            STORAGE_PUT_CF_TIME_USAGE.wal.inc_by(seconds);
            STORAGE_PUT_CF_BYTES_COUNTER.wal.inc_by(size);
        }
        StorageCategory::SignedTransaction => {
            STORAGE_PUT_CF_TIME_USAGE.signed_tx.inc_by(seconds);
            STORAGE_PUT_CF_BYTES_COUNTER.signed_tx.inc_by(size);
        }
        StorageCategory::HashHeight => {
            STORAGE_PUT_CF_TIME_USAGE.hash_height.inc_by(seconds);
            STORAGE_PUT_CF_BYTES_COUNTER.hash_height.inc_by(size);
        }
    }
}
