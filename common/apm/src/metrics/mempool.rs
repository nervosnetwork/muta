use crate::metrics::{
    auto_flush_from, exponential_buckets, make_auto_flush_static_metric, register_histogram_vec,
    register_int_counter_vec, register_int_gauge, HistogramVec, IntCounterVec, IntGauge,
};

use lazy_static::lazy_static;

make_auto_flush_static_metric! {
    pub label_enum MempoolKind {
        insert_tx_from_p2p,
        package,
        current_size,
    }

    pub label_enum MempoolOpResult {
        success,
        failure,
    }

    pub struct MempoolCounterVec: LocalIntCounter {
        "type" => MempoolKind,
    }

    pub struct MempoolResultCounterVec: LocalIntCounter {
        "type" => MempoolKind,
        "result" => MempoolOpResult,
    }

    pub struct MempoolTimeHistogramVec: LocalHistogram {
        "type" => MempoolKind,
    }

    pub struct MempoolPackageSizeVec: LocalHistogram {
        "type" => MempoolKind,
    }

    pub struct MempoolCurrentSizeVec: LocalHistogram {
        "type" => MempoolKind,
    }
}

lazy_static! {
    pub static ref MEMPOOL_COUNTER_VEC: IntCounterVec =
        register_int_counter_vec!("muta_mempool_counter", "Counter in mempool", &["type"])
            .expect("failed init mempool counter vec");
    pub static ref MEMPOOL_RESULT_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_mempool_result_counter",
        "Result counter in mempool",
        &["type", "result"]
    )
    .expect("request result total");
    pub static ref MEMPOOL_TIME_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "muta_mempool_cost_seconds",
        "Time cost in mempool",
        &["type"],
        exponential_buckets(0.05, 2.0, 10).expect("mempool time expontial")
    )
    .expect("mempool time cost");
    pub static ref MEMPOOL_PACKAGE_SIZE_VEC: HistogramVec = register_histogram_vec!(
        "muta_mempool_package_size_vec",
        "Package size",
        &["type"],
        exponential_buckets(0.05, 2.0, 10).expect("mempool package size exponential")
    )
    .expect("mempool package size");
    pub static ref MEMPOOL_CURRENT_SIZE_VEC: HistogramVec = register_histogram_vec!(
        "muta_mempool_current_size_vec",
        "Current size",
        &[],
        exponential_buckets(0.05, 2.0, 10).expect("mempool current size exponential")
    )
    .expect("mempool current size");
    pub static ref MEMPOOL_LEN_GAUGE: IntGauge =
        register_int_gauge!("muta_mempool_tx_count", "Tx len in mempool").unwrap();
}

lazy_static! {
    pub static ref MEMPOOL_COUNTER_STATIC: MempoolCounterVec =
        auto_flush_from!(MEMPOOL_COUNTER_VEC, MempoolCounterVec);
    pub static ref MEMPOOL_RESULT_COUNTER_STATIC: MempoolResultCounterVec =
        auto_flush_from!(MEMPOOL_RESULT_COUNTER_VEC, MempoolResultCounterVec);
    pub static ref MEMPOOL_TIME_STATIC: MempoolTimeHistogramVec =
        auto_flush_from!(MEMPOOL_TIME_HISTOGRAM_VEC, MempoolTimeHistogramVec);
    pub static ref MEMPOOL_PACKAGE_SIZE_VEC_STATIC: MempoolPackageSizeVec =
        auto_flush_from!(MEMPOOL_PACKAGE_SIZE_VEC, MempoolPackageSizeVec);
    pub static ref MEMPOOL_CURRENT_SIZE_VEC_STATIC: MempoolCurrentSizeVec =
        auto_flush_from!(MEMPOOL_CURRENT_SIZE_VEC, MempoolCurrentSizeVec);
}
