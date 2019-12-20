use log::{error, trace};
use moodyblues_sdk::point::{Metadata, TracePoint};
use moodyblues_sdk::time::now;
use moodyblues_sdk::trace::{set_boxed_tracer, Trace};
use serde_json::{json, to_string, to_value};

use protocol::{ProtocolError, ProtocolResult};

use crate::ConsensusError;

struct MetricTracer {
    address: String,
}

impl MetricTracer {
    fn new(address: String) -> MetricTracer {
        MetricTracer { address }
    }
}

fn err() -> ProtocolError {
    ConsensusError::Other("tracing: failed when parse point".to_string()).into()
}

fn to_trace_str(point: TracePoint) -> ProtocolResult<String> {
    let mut json = to_value(point).map_err(|_| err())?;
    let map = json.as_object_mut().ok_or_else(err)?;
    // metrics logger always takes a `name` to distinguish different metric log
    map.insert("name".to_string(), json!("moodyblues"));
    to_string(&map).map_err(|_| err())
}

impl Trace for MetricTracer {
    fn report(&self, point: TracePoint) {
        match to_trace_str(point) {
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
