use crate::metrics::{
    auto_flush_from, exponential_buckets, make_auto_flush_static_metric, register_histogram_vec,
    register_int_counter_vec, HistogramVec, IntCounterVec,
};

use lazy_static::lazy_static;

make_auto_flush_static_metric! {
    pub label_enum RequestKind {
        send_transaction,
        get_block,
    }

    pub label_enum SendTransactionResult {
        success,
        failure,
    }

    pub struct RequestCounterVec: LocalIntCounter {
        "type" => RequestKind,
    }

    pub struct RequestResultCounterVec: LocalIntCounter {
        "type" => RequestKind,
        "result" => SendTransactionResult,
    }

    pub struct RequestTimeHistogramVec: LocalHistogram {
        "type" => RequestKind,
    }
}

lazy_static! {
    pub static ref API_REQUEST_COUNTER_VEC: IntCounterVec =
        register_int_counter_vec!("muta_api_request_total", "Total number of request", &[
            "type"
        ])
        .expect("request total");
    pub static ref API_REQUEST_RESULT_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_api_request_result_total",
        "Total number of request result",
        &["type", "result"]
    )
    .expect("request result total");
    pub static ref API_REQUEST_TIME_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "muta_api_request_time_cost_seconds",
        "Request process time cost",
        &["type"],
        exponential_buckets(0.001, 2.0, 20).expect("api req time expontial")
    )
    .expect("request time cost");
}

lazy_static! {
    pub static ref API_REQUEST_COUNTER_VEC_STATIC: RequestCounterVec =
        auto_flush_from!(API_REQUEST_COUNTER_VEC, RequestCounterVec);
    pub static ref API_REQUEST_RESULT_COUNTER_VEC_STATIC: RequestResultCounterVec =
        auto_flush_from!(API_REQUEST_RESULT_COUNTER_VEC, RequestResultCounterVec);
    pub static ref API_REQUEST_TIME_HISTOGRAM_STATIC: RequestTimeHistogramVec =
        auto_flush_from!(API_REQUEST_TIME_HISTOGRAM_VEC, RequestTimeHistogramVec);
}
