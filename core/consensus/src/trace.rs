use log::error;
use moodyblues_sdk::time::now;
use moodyblues_sdk::trace::{set_boxed_tracer, Metadata, Trace, TracePoint};
use serde_json::to_string;

struct MetricTracer {
    address: String,
}

impl MetricTracer {
    fn new(address: String) -> MetricTracer {
        MetricTracer { address }
    }
}

impl Trace for MetricTracer {
    fn report(&self, point: TracePoint) {
        match to_string(&point) {
            Ok(json) => log::trace!(target: "metrics", "{}", json),
            Err(e) => error!("tracing: convert json error {:?}", e),
        }
    }

    fn metadata(&self) -> Metadata {
        Metadata {
            address: self.address.clone(),
        }
    }

    fn now(&self) -> u64 {
        now()
    }
}

pub fn init_tracer(address: String) {
    if set_boxed_tracer(Box::new(MetricTracer::new(address))).is_err() {
        error!("tracing: tracing init failed");
    }
}
