use common_apm::lazy_static::*;
use common_apm::prometheus::*;

lazy_static! {
    pub static ref ENGINE_ROUND_GAUGE: IntGauge =
        register_int_gauge!("muta_consensus_round", "Round count of consensus").unwrap();
    pub static ref ENGINE_HEIGHT_GAUGE: IntGauge =
        register_int_gauge!("muta_consensus_height", "Height of muta").unwrap();
    pub static ref ENGINE_COMMITED_TX_COUNTER: IntCounter = register_int_counter!(
        "muta_consensus_committed_tx_total",
        "The committed transactions"
    )
    .unwrap();
    pub static ref ENGINE_SYNC_BLOCK_COUNTER: IntCounter = register_int_counter!(
        "muta_consensus_sync_block_total",
        "The counter for sync blocks from remote"
    )
    .unwrap();
    pub static ref ENGINE_CONSENSUS_COST_TIME: Histogram = register_histogram!(
        "muta_consensus_duration_seconds",
        "Consensus duration from last block",
        exponential_buckets(1.0, 1.2, 15).expect("consensus duration time exponential")
    )
    .unwrap();
}
