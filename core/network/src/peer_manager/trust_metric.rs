use parking_lot::RwLock;

use std::{
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
    sync::Arc,
    time::Duration,
};

pub const PROPORTIONAL_WEIGHT: f64 = 0.1;
pub const INTERGRAL_WEIGHT: f64 = 0.9;
pub const OPTIMISTIC_HISTORY_WEIGHT: f64 = 0.9;
pub const DERIVATIVE_POSITIVE_WEIGHT: f64 = 0.05;
pub const DERIVATIVE_NEGATIVE_WEIGHT: f64 = 0.2;

pub const DEFAULT_INTERVAL_DURATION: Duration = Duration::from_secs(30);
pub const DEFAULT_MAX_HISTORY_DURATION: Duration = Duration::from_secs(24 * 60 * 60); // 1 day

// HISTORY_VLAUE_WEIGHTS are only determined by max_intervals and
// OPTIMISTIC_HISTORY_WEIGHT. Right now, all peers share same configuration, so
// we can calculate these values once.
lazy_static::lazy_static! {
    static ref HISTORY_TRUST_WEIGHTS: Arc<RwLock<Vec<f64>>> = Arc::new(RwLock::new(Vec::new()));
    static ref HISTORY_TRUST_WEIGHTS_SUM: Arc<RwLock<f64>> = Arc::new(RwLock::new(0f64));
}

pub struct TrustMetricConfig {
    interval:          Duration,
    max_history:       Duration,
    max_intervals:     u64,
    max_faded_memorys: u64,
}

impl TrustMetricConfig {
    pub fn interval(mut self, duration: Duration) -> Self {
        self.interval = duration;
        self.update();
        self
    }

    pub fn max_history(mut self, duration: Duration) -> Self {
        self.max_history = duration;
        self.update();
        self
    }

    fn update(&mut self) {
        self.max_intervals = (self.max_history.as_secs() / self.interval.as_secs());
        self.max_faded_memorys = ((self.max_intervals as f64).log2().floor() as u64) + 1;

        {
            *HISTORY_TRUST_WEIGHTS.write() = (1..=self.max_intervals)
                .map(|k| OPTIMISTIC_HISTORY_WEIGHT.powf(k as f64 - 1.0f64))
                .collect::<Vec<_>>();
        }

        {
            *HISTORY_TRUST_WEIGHTS_SUM.write() = HISTORY_TRUST_WEIGHTS.read().iter().sum();
        }
    }
}

impl Default for TrustMetricConfig {
    fn default() -> Self {
        let mut config = TrustMetricConfig {
            interval:          DEFAULT_INTERVAL_DURATION,
            max_history:       DEFAULT_MAX_HISTORY_DURATION,
            max_intervals:     0,
            max_faded_memorys: 0,
        };

        config.update();
        config
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FadedMemory(f64);

impl Deref for FadedMemory {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FadedMemory {
    pub fn new(history_value: f64) -> Self {
        FadedMemory(history_value)
    }
}

#[derive(Debug)]
struct History {
    max_intervals:   u64,
    max_memorys:     u64,
    intervals:       u64,
    memorys:         Vec<FadedMemory>,
    aggregate_trust: f64,
}

impl History {
    pub fn new(max_intervals: u64, max_memorys: u64) -> History {
        History {
            max_intervals,
            max_memorys,
            intervals: 0,
            memorys: Vec::new(),
            aggregate_trust: 0f64,
        }
    }

    fn remember_interval(&mut self, trust_value: f64) {
        if self.intervals < self.max_intervals {
            self.intervals += 1;
        }

        if self.intervals <= self.max_memorys {
            self.memorys.insert(0, FadedMemory::new(trust_value));
            return;
        }

        let memento = self.memorys.len();
        self.memorys.insert(0, FadedMemory::new(trust_value));
        self.memorys = (1..=memento)
            .map(|j| {
                let ftv = (*self.memorys[j - 1]
                    + (*self.memorys[j] * (2f64.powf(j as f64) - 1f64)))
                    / 2f64.powf(j as f64);
                FadedMemory::new(ftv)
            })
            .collect::<Vec<_>>();
    }

    fn update_aggregate_trust(&mut self) {
        let intervals = self.intervals;
        if intervals < 2 {
            return;
        }

        self.aggregate_trust = (1..=intervals).map(|i| {
            let memory_idx = (i as f64).log2().floor() as usize;
            let i_hist_trust = match self.memorys.get(memory_idx).cloned() {
                Some(v) => *v,
                None => {
                    log::error!(target: "p2p-trust-metric", "history interval {} trust value not found", i);
                    0f64
                }
            };
            let i_hist_weight = match HISTORY_TRUST_WEIGHTS.read().get(i as usize).cloned() {
                Some(v) => v,
                None => {
                    log::error!(target: "p2p-trust-metric", "history interval {} weight not found", i);
                    0f64
                }
            };

            i_hist_trust * i_hist_weight / HISTORY_TRUST_WEIGHTS_SUM.read().to_owned()
        }).sum::<f64>();
    }
}

pub struct TrustMetric {
    config:      Arc<TrustMetricConfig>,
    history:     RwLock<History>,
    good_events: AtomicUsize,
    bad_events:  AtomicUsize,
}

impl TrustMetric {
    pub fn new(config: Arc<TrustMetricConfig>) -> Self {
        let max_intervals = config.max_intervals;
        let max_memorys = config.max_faded_memorys;

        TrustMetric {
            config,
            history: RwLock::new(History::new(max_intervals, max_memorys)),
            good_events: AtomicUsize::new(0),
            bad_events: AtomicUsize::new(0),
        }
    }

    pub fn trust_score(&self) -> u8 {
        (self.trust_value() * 100f64) as u8 + 1
    }

    pub fn good_events(&self, num: usize) {
        self.good_events.fetch_add(num, SeqCst);
    }

    pub fn bad_events(&self, num: usize) {
        self.bad_events.fetch_add(num, SeqCst);
    }

    pub fn enter_new_interval(&self) {
        let trust_value = self.trust_value();
        log::debug!(target: "p2p-trust-metric", "enter new interval, passing trust value {}", trust_value);

        {
            let mut history = self.history.write();
            history.remember_interval(trust_value);
            history.update_aggregate_trust();
        }

        self.good_events.store(0, SeqCst);
        self.bad_events.store(0, SeqCst);
    }

    fn trust_value(&self) -> f64 {
        let proportional_value = self.proportional_value();
        let intergral_value = self.intergral_value();
        let deviation_value = proportional_value - intergral_value;
        let derivative_value = if deviation_value >= 0f64 {
            DERIVATIVE_POSITIVE_WEIGHT * deviation_value
        } else {
            DERIVATIVE_NEGATIVE_WEIGHT * deviation_value
        };

        proportional_value + intergral_value + derivative_value
    }

    fn proportional_value(&self) -> f64 {
        let base = 1.0;
        let good_events = self.good_events.load(SeqCst);
        let total = good_events + self.bad_events.load(SeqCst);

        if total > 0 {
            (good_events / total) as f64 * PROPORTIONAL_WEIGHT
        } else {
            base * PROPORTIONAL_WEIGHT
        }
    }

    fn intergral_value(&self) -> f64 {
        self.history.read().aggregate_trust * INTERGRAL_WEIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::{TrustMetric, TrustMetricConfig};

    use std::sync::Arc;

    #[test]
    fn basic_metric_test() {
        let config = Arc::new(TrustMetricConfig::default());
        let metric = TrustMetric::new(config.clone());

        for _ in (0..config.max_intervals) {
            metric.good_events(10);
            metric.enter_new_interval();
        }
        assert_eq!(metric.trust_score(), 100);
    }
}
