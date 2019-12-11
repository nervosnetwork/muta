use common_logger::{metrics, object};
use moodyblues_sdk::time::now;
use moodyblues_sdk::trace::{start_epoch, set_boxed_tracer, Metadata, Trace, TracePoint};
use serde_json::to_string;

pub struct MetricTracer {
    address: String,
}

impl MetricTracer {
    fn new(address: String) -> MetricTracer {
        MetricTracer { address }
    }
}

impl Trace for MetricTracer {
    fn report(&self, point: TracePoint) {
        log::trace!(target: "metrics", "{}", to_string(&point).unwrap());
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
    set_boxed_tracer(Box::new(MetricTracer::new(address)));
    start_epoch(1);
}
