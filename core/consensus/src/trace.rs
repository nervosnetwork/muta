use log::{error, trace};
use moodyblues_sdk::point::{Metadata, TracePoint};
use moodyblues_sdk::time::now;
use moodyblues_sdk::trace::{set_boxed_tracer, Trace};
use serde_json::to_string;

use protocol::ProtocolResult;

use crate::ConsensusError;

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
            Ok(json) => trace!(target: "metrics", "{}", json),
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

pub fn init_tracer(address: String) -> ProtocolResult<()> {
    set_boxed_tracer(Box::new(MetricTracer::new(address)))
        .map_err(|_| ConsensusError::Other("failed to init tracer ".to_string()).into())
}
