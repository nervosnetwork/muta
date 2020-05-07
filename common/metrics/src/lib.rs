pub mod api;

use derive_more::Display;
use prometheus::{Encoder, TextEncoder};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub use prometheus::{
    register_histogram, register_histogram_vec, register_int_counter, register_int_counter_vec,
    register_int_gauge, register_int_gauge_vec, Histogram, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, IntGaugeVec,
};

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

pub struct DurationHistogram(Histogram);

impl DurationHistogram {
    pub fn new(histogram: Histogram) -> DurationHistogram {
        DurationHistogram(histogram)
    }

    pub fn observe_duration(&self, d: Duration) {
        // Duration is full seconds + nanos elapsed from the previous full second
        let v = d.as_secs_f64() + f64::from(d.subsec_nanos()) / 1e9;
        self.0.observe(v);
    }
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
