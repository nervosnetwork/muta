use futures::{
    future::{self, AbortHandle},
    pin_mut,
};
use futures_timer::Delay;
use parking_lot::RwLock;

use std::{
    future::Future,
    ops::{Add, Deref},
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

pub const PROPORTIONAL_WEIGHT: f64 = 0.4;
pub const INTERGRAL_WEIGHT: f64 = 0.6;
pub const OPTIMISTIC_HISTORY_WEIGHT: f64 = 0.8;
pub const DERIVATIVE_POSITIVE_WEIGHT: f64 = 0.0;
pub const DERIVATIVE_NEGATIVE_WEIGHT: f64 = 0.1;

pub const KNOCK_OUT_SCORE: u8 = 30;

pub const DEFAULT_INTERVAL_DURATION: Duration = Duration::from_secs(60);
pub const DEFAULT_MAX_HISTORY_DURATION: Duration = Duration::from_secs(24 * 60 * 60 * 10); // 10 day

// HISTORY_VLAUE_WEIGHTS are only determined by max_intervals and
// OPTIMISTIC_HISTORY_WEIGHT. Right now, all peers share same configuration, so
// we can calculate these values once.
lazy_static::lazy_static! {
    static ref HISTORY_TRUST_WEIGHTS: Arc<RwLock<Vec<f64>>> = Arc::new(RwLock::new(Vec::new()));
}

#[derive(Debug)]
pub struct TrustMetricConfig {
    interval:          Duration,
    max_history:       Duration,
    max_intervals:     u64,
    max_faded_memorys: u64,
}

impl TrustMetricConfig {
    pub fn new(interval: Duration, max_history: Duration) -> Self {
        let partial_config = TrustMetricConfig {
            interval,
            max_history,
            max_intervals: 0,
            max_faded_memorys: 0,
        };

        partial_config.finish()
    }

    fn finish(mut self) -> Self {
        self.max_intervals = self.max_history.as_secs() / self.interval.as_secs();
        self.max_faded_memorys = ((self.max_intervals as f64).log2().floor() as u64) + 1;
        log::debug!(target: "network-trust-metric", "max intervals {}", self.max_intervals);
        log::debug!(target: "network-trust-metric", "max faded memorys {}", self.max_faded_memorys);

        {
            *HISTORY_TRUST_WEIGHTS.write() = (1..=self.max_intervals)
                .map(|k| OPTIMISTIC_HISTORY_WEIGHT.powf((k - 1) as f64))
                .collect::<Vec<_>>();
        }

        self
    }
}

impl Default for TrustMetricConfig {
    fn default() -> Self {
        let partial_config = TrustMetricConfig {
            interval:          DEFAULT_INTERVAL_DURATION,
            max_history:       DEFAULT_MAX_HISTORY_DURATION,
            max_intervals:     0,
            max_faded_memorys: 0,
        };

        partial_config.finish()
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
    fn new(history_value: f64) -> Self {
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
    weights_sum:     f64,
}

impl History {
    fn new(max_intervals: u64, max_memorys: u64) -> History {
        History {
            max_intervals,
            max_memorys,
            intervals: 0,
            memorys: Vec::new(),
            aggregate_trust: 0f64,
            weights_sum: 0f64,
        }
    }

    fn remember_interval(&mut self, trust_value: f64) {
        if self.intervals < self.max_intervals {
            self.intervals += 1;

            let i = self.intervals;
            self.weights_sum += match HISTORY_TRUST_WEIGHTS.read().get(i as usize - 1).cloned() {
                Some(v) => v,
                None => {
                    log::warn!(target: "network-trust-metric", "precalculated history interval {} trust weight not found", i);
                    OPTIMISTIC_HISTORY_WEIGHT.powf((i - 1) as f64)
                }
            };
        }

        if self.intervals <= self.max_memorys {
            self.memorys.insert(0, FadedMemory::new(trust_value));
            return;
        }

        // Update faded memorys
        let memento = self.memorys.len() - 1;
        self.memorys = (1..=memento)
            .map(|j| {
                let w = 2f64.powf(j as f64);
                let ftv = (*self.memorys[j - 1] + (*self.memorys[j] * (w - 1f64))) / w;
                FadedMemory::new(ftv)
            })
            .collect::<Vec<_>>();
        self.memorys.insert(0, FadedMemory::new(trust_value));
    }

    fn update_aggregate_trust(&mut self) {
        let intervals = self.intervals;
        if intervals < 1 {
            return;
        }

        self.aggregate_trust = (1..=intervals).map(|i| {
            let memory_idx = (i as f64).log2().floor() as usize;

            let i_hist_trust = match self.memorys.get(memory_idx).cloned() {
                Some(v) => *v,
                None => {
                    log::error!(target: "network-trust-metric", "history interval {} trust value not found", i);
                    0f64
                }
            };
            let i_hist_weight = match HISTORY_TRUST_WEIGHTS.read().get(i as usize - 1).cloned() {
                Some(v) => v,
                None => {
                    log::warn!(target: "network-trust-metric", "precalculated history interval {} weight not found", i);
                    OPTIMISTIC_HISTORY_WEIGHT.powf((i - 1) as f64)
                }
            };

            i_hist_trust * (i_hist_weight / self.weights_sum)
        }).sum::<f64>();

        log::debug!(target: "network-trust-metric", "aggregate trust {}", self.aggregate_trust);
    }
}

#[derive(Debug)]
pub struct Inner {
    config:      Arc<TrustMetricConfig>,
    history:     RwLock<History>,
    good_events: AtomicUsize,
    bad_events:  AtomicUsize,
}

impl Inner {
    pub fn new(config: Arc<TrustMetricConfig>) -> Self {
        let max_intervals = config.max_intervals;
        let max_memorys = config.max_faded_memorys;

        Inner {
            config,
            history: RwLock::new(History::new(max_intervals, max_memorys)),
            good_events: AtomicUsize::new(0),
            bad_events: AtomicUsize::new(0),
        }
    }

    pub fn trust_score(&self) -> u8 {
        (self.trust_value() * 100f64) as u8
    }

    pub fn good_events(&self, num: usize) {
        self.good_events.fetch_add(num, SeqCst);
    }

    pub fn bad_events(&self, num: usize) {
        self.bad_events.fetch_add(num, SeqCst);
    }

    pub fn knock_out(&self) -> bool {
        self.trust_score() < KNOCK_OUT_SCORE
    }

    pub fn enter_new_interval(&self) {
        let latest_trust_value = self.trust_value();
        log::debug!(target: "network-trust-metric", "enter new interval, lastest trust value {}", latest_trust_value);

        {
            let mut history = self.history.write();
            history.remember_interval(latest_trust_value);
            history.update_aggregate_trust();
        }

        self.good_events.store(0, SeqCst);
        self.bad_events.store(0, SeqCst);
    }

    pub fn reset_history(&self) {
        let max_intervals = self.config.max_intervals;
        let max_memorys = self.config.max_faded_memorys;

        *self.history.write() = History::new(max_intervals, max_memorys);
    }

    fn trust_value(&self) -> f64 {
        let proportional_value = match self.proportional_value() {
            Some(v) => v,
            None => return self.history.read().aggregate_trust,
        };

        let intergral_value = self.intergral_value();
        let deviation_value = proportional_value - intergral_value;
        let derivative_value = if deviation_value >= 0f64 {
            DERIVATIVE_POSITIVE_WEIGHT * deviation_value
        } else {
            DERIVATIVE_NEGATIVE_WEIGHT * deviation_value
        };

        log::debug!(target: "network-trust-metric", "trust value components: r {:?}, h {}, d {}", proportional_value, intergral_value, derivative_value);
        proportional_value + intergral_value + derivative_value
    }

    fn proportional_value(&self) -> Option<f64> {
        let good_events = self.good_events.load(SeqCst);
        let total = good_events + self.bad_events.load(SeqCst);

        if total > 0 {
            Some((good_events as f64 / total as f64) * PROPORTIONAL_WEIGHT)
        } else {
            None
        }
    }

    fn intergral_value(&self) -> f64 {
        self.history.read().aggregate_trust * INTERGRAL_WEIGHT
    }
}

struct HeartBeat {
    inner:          Arc<Inner>,
    interval:       Duration,
    delay:          Delay,
    pause_save:     Arc<RwLock<Option<Duration>>>,
    interval_start: Instant,
}

impl HeartBeat {
    pub fn new(
        inner: Arc<Inner>,
        interval: Duration,
        resume: Option<Duration>,
        pause_save: Arc<RwLock<Option<Duration>>>,
    ) -> Self {
        let delay = match resume {
            Some(resume) => {
                let remain = interval - resume;
                if remain.as_secs() > 0 {
                    Delay::new(remain)
                } else {
                    Delay::new(interval)
                }
            }
            None => Delay::new(interval),
        };

        HeartBeat {
            inner,
            interval,
            delay,
            pause_save,
            interval_start: Instant::now(),
        }
    }
}

impl Drop for HeartBeat {
    fn drop(&mut self) {
        let elapsed = self.interval_start.elapsed();
        *self.pause_save.write() = Some(elapsed);
    }
}

impl Future for HeartBeat {
    type Output = <Delay as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let ecg = &mut self.as_mut();

        loop {
            let interval = ecg.interval;
            let delay = &mut ecg.delay;
            pin_mut!(delay);

            crate::loop_ready!(delay.poll(ctx));
            ecg.inner.enter_new_interval();
            ecg.interval_start = Instant::now();

            let next_interval = Instant::now().add(interval);
            ecg.delay.reset(next_interval);
        }

        Poll::Pending
    }
}

#[derive(Debug, Clone)]
pub struct TrustMetric {
    inner:     Arc<Inner>,
    hb_handle: Arc<RwLock<Option<AbortHandle>>>,
    pause:     Arc<RwLock<Option<Duration>>>,
}

impl TrustMetric {
    pub fn new(config: Arc<TrustMetricConfig>) -> Self {
        TrustMetric {
            inner:     Arc::new(Inner::new(config)),
            hb_handle: Arc::new(RwLock::new(None)),
            pause:     Arc::new(RwLock::new(None)),
        }
    }

    pub fn start(&self) {
        if self.hb_handle.read().is_some() {
            // Already started
            return;
        }

        let interval = self.inner.config.interval;
        let resume = self.pause.write().take();
        let heart_beat = HeartBeat::new(
            Arc::clone(&self.inner),
            interval,
            resume,
            Arc::clone(&self.pause),
        );

        let (heart_beat, hb_handle) = future::abortable(heart_beat);
        *self.hb_handle.write() = Some(hb_handle);
        tokio::spawn(heart_beat);
    }

    pub fn pause(&self) {
        if let Some(abort_handle) = self.hb_handle.write().take() {
            abort_handle.abort();
        }
    }
}

impl Deref for TrustMetric {
    type Target = Arc<Inner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::{Inner, TrustMetricConfig};

    use std::sync::Arc;

    #[test]
    fn basic_metric_test() {
        env_logger::init();

        let config = Arc::new(TrustMetricConfig::default());
        let metric = Inner::new(config);

        for _ in 0..20 {
            metric.good_events(1);
            metric.enter_new_interval();
        }
        assert!(metric.trust_score() > 90);

        for _ in 0..5 {
            metric.bad_events(1);
            metric.enter_new_interval();
        }
        assert!(metric.trust_score() < 70);

        for _ in 0..20 {
            metric.good_events(1);
            metric.enter_new_interval();
        }
        assert!(metric.trust_score() > 80 && metric.trust_score() < 90);
    }
}
