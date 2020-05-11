use crate::metrics::{
    auto_flush_from, exponential_buckets, make_auto_flush_static_metric, register_histogram_vec,
    register_int_counter_vec, HistogramVec, IntCounterVec,
};

use lazy_static::lazy_static;

make_auto_flush_static_metric! {
    pub label_enum SyncKind {
        sync,
    }

    pub label_enum SyncResult {
        success,
        failure,
    }

    pub struct SyncCounterVec: LocalIntCounter {
        "type" => SyncKind,
    }

    pub struct SyncResultCounterVec: LocalIntCounter {
        "type" => SyncKind,
        "result" => SyncResult,
    }

    pub struct SyncTimeHistogramVec: LocalHistogram {
        "type" => SyncKind,
    }
}

lazy_static! {
    pub static ref SYNC_COUNTER_VEC: IntCounterVec =
        register_int_counter_vec!("muta_sync_total", "Counts of sync", &["type"])
            .expect("sync counter");
    pub static ref SYNC_RESULT_COUNTER_VEC: IntCounterVec =
        register_int_counter_vec!("muta_sync_result_total", "Total number of sync result", &[
            "type", "result"
        ])
        .expect("sync result total");
    pub static ref SYNC_TIME_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "muta_api_request_time_cost_seconds",
        "Request process time cost",
        &["type"],
        exponential_buckets(1.0, 2.0, 10).expect("api req time expontial")
    )
    .expect("request time cost");
}

lazy_static! {
    pub static ref SYNC_COUNTER_VEC_STATIC: SyncCounterVec =
        auto_flush_from!(SYNC_COUNTER_VEC, SyncCounterVec);
    pub static ref SYNC_RESULT_COUNTER_VEC_STATIC: SyncResultCounterVec =
        auto_flush_from!(SYNC_RESULT_COUNTER_VEC, SyncResultCounterVec);
    pub static ref SYNC_TIME_HISTOGRAM_VEC_STATIC: SyncTimeHistogramVec =
        auto_flush_from!(SYNC_TIME_HISTOGRAM_VEC, SyncTimeHistogramVec);
}
