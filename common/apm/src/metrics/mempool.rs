use crate::metrics::{
    auto_flush_from, exponential_buckets, make_auto_flush_static_metric, register_histogram_vec,
    register_int_counter_vec, register_int_gauge, HistogramVec, IntCounterVec, IntGauge,
};

use lazy_static::lazy_static;

make_auto_flush_static_metric! {
    pub label_enum MempoolCounterKind {
        insert_tx_from_p2p,
        package,
    }

    pub struct MempoolCounterVec: LocalIntCounter {
        "type" => MempoolCounterKind,
    }

    pub label_enum MempoolOpResult {
        success,
        failure,
    }

    pub struct MempoolResultCounterVec: LocalIntCounter {
        "type" => MempoolCounterKind,
        "result" => MempoolOpResult,
    }

    pub struct MempoolTimeHistogramVec: LocalHistogram {
        "type" => MempoolCounterKind,
    }
}

lazy_static! {
    pub static ref MEMPOOL_COUNTER_VEC: IntCounterVec =
        register_int_counter_vec!("muta_mempool_counter", "Counter in mempool", &["type"]).unwrap();
    pub static ref MEMPOOL_RESULT_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_mempool_result_counter",
        "Result counter in mempool",
        &["type", "result"]
    )
    .unwrap();
    pub static ref MEMPOOL_TIME_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "muta_mempool_cost_seconds",
        "Time cost in mempool",
        &["type"],
        exponential_buckets(0.05, 1.5, 10).unwrap()
    )
    .unwrap();
    pub static ref MEMPOOL_PACKAGE_SIZE_STATIC: IntGauge =
        register_int_gauge!("muta_mempool_package_size", "Package size").unwrap();
    pub static ref MEMPOOL_CURRENT_SIZE_STATIC: IntGauge =
        register_int_gauge!("muta_mempool_current_size", "Current size").unwrap();
}

lazy_static! {
    pub static ref MEMPOOL_COUNTER_STATIC: MempoolCounterVec =
        auto_flush_from!(MEMPOOL_COUNTER_VEC, MempoolCounterVec);
    pub static ref MEMPOOL_RESULT_COUNTER_STATIC: MempoolResultCounterVec =
        auto_flush_from!(MEMPOOL_RESULT_COUNTER_VEC, MempoolResultCounterVec);
    pub static ref MEMPOOL_TIME_STATIC: MempoolTimeHistogramVec =
        auto_flush_from!(MEMPOOL_TIME_HISTOGRAM_VEC, MempoolTimeHistogramVec);
    // pub static ref MEMPOOL_SIZE_VEC_STATIC: MempoolSizeVec =
    //     auto_flush_from!(MEMPOOL_SIZE_VEC, MempoolSizeVec);
}
