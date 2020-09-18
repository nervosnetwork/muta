use crate::metrics::{
    auto_flush_from, exponential_buckets, make_auto_flush_static_metric, register_histogram,
    register_histogram_vec, register_int_counter, register_int_counter_vec, register_int_gauge,
    Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge,
};

use lazy_static::lazy_static;

make_auto_flush_static_metric! {
    pub label_enum ConsensusResultKind {
        get_block_from_remote,
    }

    pub label_enum ConsensusResult {
        success,
        failure,
    }

    pub struct ConsensusResultCounterVec: LocalIntCounter {
        "type" => ConsensusResultKind,
        "result" => ConsensusResult,
    }

    pub label_enum ConsensusTimeKind {
        commit,
        exec,
        block
    }

    pub struct ConsensusTimeHistogramVec: LocalHistogram {
        "type" => ConsensusTimeKind,
    }

    pub label_enum ConsensusRoundKind {
        round
    }

    pub struct ConsensusRoundHistogramVec: LocalHistogram {
        "type" => ConsensusRoundKind,
    }
}

lazy_static! {
    pub static ref CONSENSUS_RESULT_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_concensus_result",
        "Total number of consensus result",
        &["type", "result"]
    )
    .unwrap();
    pub static ref CONSENSUS_TIME_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "muta_consensus_time_cost_seconds",
        "Consensus process time cost",
        &["type"],
        exponential_buckets(0.05, 1.2, 30).unwrap()
    )
    .unwrap();
}

lazy_static! {
    pub static ref CONSENSUS_RESULT_COUNTER_VEC_STATIC: ConsensusResultCounterVec =
        auto_flush_from!(CONSENSUS_RESULT_COUNTER_VEC, ConsensusResultCounterVec);
    pub static ref CONSENSUS_TIME_HISTOGRAM_VEC_STATIC: ConsensusTimeHistogramVec =
        auto_flush_from!(CONSENSUS_TIME_HISTOGRAM_VEC, ConsensusTimeHistogramVec);
    pub static ref ENGINE_ROUND_GAUGE: IntGauge =
        register_int_gauge!("muta_consensus_round", "Round count of consensus").unwrap();
    pub static ref ENGINE_HEIGHT_GAUGE: IntGauge =
        register_int_gauge!("muta_consensus_height", "Height of muta").unwrap();
    pub static ref ENGINE_EXECUTING_BLOCK_GAUGE: IntGauge =
        register_int_gauge!("muta_executing_block_count", "The executing blocks").unwrap();
    pub static ref ENGINE_COMMITED_TX_COUNTER: IntCounter = register_int_counter!(
        "muta_consensus_committed_tx_total",
        "The committed transactions"
    )
    .unwrap();
    pub static ref ENGINE_ORDER_TX_GAUGE: IntGauge =
        register_int_gauge!("muta_proposal_order_tx_len", "The ordered transactions len").unwrap();
    pub static ref ENGINE_SYNC_TX_GAUGE: IntGauge =
        register_int_gauge!("muta_proposal_sync_tx_len", "The sync transactions len").unwrap();
    pub static ref ENGINE_SYNC_BLOCK_COUNTER: IntCounter = register_int_counter!(
        "muta_consensus_sync_block_total",
        "The counter for sync blocks from remote"
    )
    .unwrap();
    pub static ref ENGINE_SYNC_BLOCK_HISTOGRAM: Histogram = register_histogram!(
        "muta_consensus_sync_block_duration",
        "Histogram of consensus sync duration",
        exponential_buckets(0.5, 1.2, 20).expect("consensus duration time exponential")
    )
    .unwrap();
    pub static ref ENGINE_CONSENSUS_COST_TIME: Histogram = register_histogram!(
        "muta_consensus_duration_seconds",
        "Histogram of consensus duration from last block",
        exponential_buckets(1.0, 1.2, 15).expect("consensus duration time exponential")
    )
    .unwrap();
}
