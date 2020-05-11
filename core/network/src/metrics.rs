use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures::task::AtomicWaker;

use crate::{common::HeartBeat, traits::SessionBook};

const METRICS_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct Metrics<S> {
    sessions:   S,
    heart_beat: Option<HeartBeat>,
    hb_waker:   Arc<AtomicWaker>,
}

impl<S> Metrics<S>
where
    S: SessionBook + Send + Unpin + 'static,
{
    pub fn new(sessions: S) -> Self {
        let waker = Arc::new(AtomicWaker::new());
        let heart_beat = HeartBeat::new(Arc::clone(&waker), METRICS_INTERVAL);

        Metrics {
            sessions,
            heart_beat: Some(heart_beat),
            hb_waker: waker,
        }
    }

    fn report_pending_data(&self) {
        let sids = self.sessions.all();

        let total_size: usize = sids
            .iter()
            .map(|sid| self.sessions.pending_data_size(*sid))
            .sum();

        common_apm::metrics::network::NETWORK_PENDING_DATA_SIZE.set(total_size as i64);
    }
}

impl<S> Future for Metrics<S>
where
    S: SessionBook + Send + Unpin + 'static,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self.hb_waker.register(ctx.waker());

        // Spawn heart beat
        if let Some(heart_beat) = self.heart_beat.take() {
            tokio::spawn(heart_beat);

            // No needed for first run
            return Poll::Pending;
        }

        self.as_ref().report_pending_data();

        Poll::Pending
    }
}
