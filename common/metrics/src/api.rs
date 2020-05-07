use super::{register_histogram, register_int_counter, DurationHistogram, IntCounter};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref TX_COUNT: IntCounter =
        register_int_counter!("muta_api_raw_tx_count", "Raw tx count").expect("api tx count");
    pub static ref SUCCESS_TX_COUNT: IntCounter =
        register_int_counter!("muta_api_success_tx_count", "Success tx count")
            .expect("api success tx count");
    pub static ref REPEATED_TX_COUNT: IntCounter =
        register_int_counter!("muta_api_repeated_tx_count", "Repeated tx count")
            .expect("api repeatd tx count");
    pub static ref TX_SUCCESS_TIME_COST: DurationHistogram = DurationHistogram::new(
        register_histogram!("muta_api_tx_success_time_cost", "Tx success time cost")
            .expect("api tx success time cost")
    );
}
