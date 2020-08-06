// https://rust-lang.github.io/rust-clippy/master/index.html#float_cmp
#![allow(clippy::float_cmp)]

pub mod metrics;

pub use muta_apm;

pub use lazy_static;
pub use prometheus;
pub use prometheus_static_metric;
