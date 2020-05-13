use crate::metrics::{
    auto_flush_from, exponential_buckets, make_auto_flush_static_metric, register_histogram_vec,
    register_int_counter_vec, HistogramVec, IntCounterVec,
};

use lazy_static::lazy_static;

make_auto_flush_static_metric! {
    pub label_enum ConsensusKind {
        round,
        commit,
        exec,
        block
    }

    pub label_enum ConsensusResult {
        success,
        failure,
    }

    pub struct ConsensusCounterVec: LocalIntCounter {
        "type" => ConsensusKind,
    }

    pub struct ConsensusResultCounterVec: LocalIntCounter {
        "type" => ConsensusKind,
        "result" => ConsensusResult,
    }

    pub struct ConsensusTimeHistogramVec: LocalHistogram {
        "type" => ConsensusKind,
    }

    pub struct ConsensusInfoHistogramVec: LocalHistogram {
        "type" => ConsensusKind,
    }
}

lazy_static! {
    pub static ref CONSENSUS_COUNTER_VEC: IntCounterVec =
        register_int_counter_vec!("muta_concensus_total", "Total number of consensus", &[
            "type"
        ])
        .expect("concensus total");
    pub static ref CONSENSUS_RESULT_COUNTER_VEC: IntCounterVec = register_int_counter_vec!(
        "muta_concensus_result_total",
        "Total number of consensus result",
        &["type", "result"]
    )
    .expect("request result total");
    pub static ref CONSENSUS_TIME_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "muta_consensus_time_cost_seconds",
        "Consensus process time cost",
        &["type"],
        exponential_buckets(0.05, 1.5, 20).expect("consensus time expontial")
    )
    .expect("consensus time cost");
    pub static ref CONSENSUS_INFO_HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "muta_consensus_info",
        "Consensus extra info",
        &["type"],
        exponential_buckets(0.5, 1.5, 10).expect("consensus extra info expontial")
    )
    .expect("consensus time cost");
}

lazy_static! {
    pub static ref CONSENSUS_COUNTER_VEC_STATIC: ConsensusCounterVec =
        auto_flush_from!(CONSENSUS_COUNTER_VEC, ConsensusCounterVec);
    pub static ref CONSENSUS_RESULT_COUNTER_VEC_STATIC: ConsensusResultCounterVec =
        auto_flush_from!(CONSENSUS_RESULT_COUNTER_VEC, ConsensusResultCounterVec);
    pub static ref CONSENSUS_TIME_HISTOGRAM_VEC_STATIC: ConsensusTimeHistogramVec =
        auto_flush_from!(CONSENSUS_TIME_HISTOGRAM_VEC, ConsensusTimeHistogramVec);
    pub static ref CONSENSUS_INFO_HISTOGRAM_VEC_STATIC: ConsensusInfoHistogramVec =
        auto_flush_from!(CONSENSUS_INFO_HISTOGRAM_VEC, ConsensusInfoHistogramVec);
}
