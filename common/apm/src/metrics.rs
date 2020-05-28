pub mod api;
pub mod consensus;
pub mod mempool;
pub mod network;

pub use prometheus::{Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec};

use derive_more::Display;
use prometheus::{
    exponential_buckets, register_histogram, register_histogram_vec, register_int_counter,
    register_int_counter_vec, register_int_gauge, register_int_gauge_vec, Encoder, TextEncoder,
};
use prometheus_static_metric::{auto_flush_from, make_auto_flush_static_metric};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use std::time::Duration;

#[derive(Debug, Display)]
enum Error {
    #[display(fmt = "promtheus {}", _0)]
    Prometheus(prometheus::Error),
}

impl From<prometheus::Error> for Error {
    fn from(err: prometheus::Error) -> Error {
        Error::Prometheus(err)
    }
}

impl From<Error> for ProtocolError {
    fn from(err: Error) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Metric, Box::new(err))
    }
}

impl std::error::Error for Error {}

pub fn duration_to_sec(d: Duration) -> f64 {
    d.as_secs_f64() + (f64::from(d.subsec_nanos()) / 1e9)
}

pub fn all_metrics() -> ProtocolResult<Vec<u8>> {
    let metric_families = prometheus::gather();
    let encoder = TextEncoder::new();

    let mut encoded_metrics = vec![];
    encoder
        .encode(&metric_families, &mut encoded_metrics)
        .map_err(Error::Prometheus)?;

    Ok(encoded_metrics)
}
