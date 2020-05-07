use derive_more::Display;
use prometheus::{Encoder, TextEncoder};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub use prometheus::{
    register_histogram, register_histogram_vec, register_int_counter, register_int_counter_vec,
    register_int_gauge, register_int_gauge_vec, Histogram, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, IntGaugeVec,
};

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

pub fn all_metrics() -> ProtocolResult<Vec<u8>> {
    let metric_families = prometheus::gather();
    let encoder = TextEncoder::new();

    let mut encoded_metrics = vec![];
    encoder
        .encode(metric_families, encoded_metrics)
        .map_err(Error::Prometheus)?;

    Ok(encoded_metrics)
}
