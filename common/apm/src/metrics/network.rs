use crate::metrics::{
    auto_flush_from, exponential_buckets, make_auto_flush_static_metric, register_histogram_vec,
    register_int_counter_vec, register_int_gauge, register_int_gauge_vec, HistogramVec,
    IntCounterVec, IntGauge, IntGaugeVec,
};

use lazy_static::lazy_static;

make_auto_flush_static_metric! {
    pub label_enum MessageDirection {
        sent,
        received,
    }

    pub label_enum ProtocolKind {
        rpc,
    }

    pub label_enum RPCResult {
        success,
        timeout,
    }

    pub struct MessageCounterVec: LocalIntCounter {
        "direction" => MessageDirection,
    }

    pub struct RPCResultCounterVec: LocalIntCounter {
        "result" => RPCResult,
    }

    pub struct ProtocolTimeHistogramVec: LocalHistogram {
        "type" => ProtocolKind,
    }
}

lazy_static! {
    pub static ref NETWORK_MESSAGE_COUNT_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_network_message_total",
        "Total number of network message",
        &["direction", "type", "module", "action"]
    )
    .expect("network message total");
    pub static ref NETWORK_RPC_RESULT_COUNT_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_network_rpc_result_total",
        "Total number of network rpc result",
        &["result"]
    )
    .expect("network rpc result total");
    pub static ref NETWORK_PROTOCOL_TIME_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "muta_network_protocol_time_cost_seconds",
        "Network protocol time cost",
        &["type"],
        exponential_buckets(0.01, 2.0, 20).expect("network protocol time expontial")
    )
    .expect("network protocol time cost");
}

lazy_static! {
    pub static ref NETWORK_RPC_RESULT_COUNT_VEC_STATIC: RPCResultCounterVec =
        auto_flush_from!(NETWORK_RPC_RESULT_COUNT_VEC, RPCResultCounterVec);
    pub static ref NETWORK_PROTOCOL_TIME_HISTOGRAM_VEC_STATIC: ProtocolTimeHistogramVec = auto_flush_from!(
        NETWORK_PROTOCOL_TIME_HISTOGRAM_VEC,
        ProtocolTimeHistogramVec
    );
}

lazy_static! {
    pub static ref NETWORK_TOTAL_PENDING_DATA_SIZE: IntGauge = register_int_gauge!(
        "muta_network_total_pending_data_size",
        "Total pending data size"
    )
    .expect("network total pending data size");
    pub static ref NETWORK_IP_PENDING_DATA_SIZE_VEC: IntGaugeVec = register_int_gauge_vec!(
        "muta_network_ip_pending_data_size",
        "IP pending data size",
        &["ip"]
    )
    .expect("network ip pending data size");
}

fn on_network_message(direction: &str, url: &str) {
    let spliced: Vec<&str> = url.split("/").collect();
    if spliced.len() < 4 {
        return;
    }

    NETWORK_MESSAGE_COUNT_VEC
        .with_label_values(&[direction, spliced[1], spliced[2], spliced[3]])
        .inc();
}

pub fn on_network_message_sent(url: &str) {
    on_network_message("sent", url);
}

pub fn on_network_message_received(url: &str) {
    on_network_message("received", url);
}
