use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures::task::AtomicWaker;
use log::info;

use crate::{common::HeartBeat, traits::SharedSessionBook};

pub struct SelfCheckConfig {
    pub interval: Duration,
}

pub(crate) struct SelfCheck<S> {
    sessions:   S,
    heart_beat: Option<HeartBeat>,
    hb_waker:   Arc<AtomicWaker>,
}

impl<S> SelfCheck<S>
where
    S: SharedSessionBook + Send + Unpin + 'static,
{
    pub fn new(sessions: S, config: SelfCheckConfig) -> Self {
        let waker = Arc::new(AtomicWaker::new());
        let heart_beat = HeartBeat::new(Arc::clone(&waker), config.interval);

        SelfCheck {
            sessions,
            heart_beat: Some(heart_beat),
            hb_waker: waker,
        }
    }

    fn report_allowlist(&self) {
        info!("peers in allowlist: {:?}", self.sessions.allowlist());
    }

    fn report_pending_data(&self) {
        let sids = self.sessions.all();
        let mut total_size = 0;

        let peer_reports = sids
            .into_iter()
            .map(|sid| {
                let connected_addr = self.sessions.connected_addr(sid);
                let data_size = self.sessions.pending_data_size(sid) / (1000 * 1000); // MB not MiB

                total_size += data_size;
                (connected_addr, data_size)
            })
            .collect::<Vec<_>>();

        info!(
            "total connected peers: {}, pending size {} MB, session(s) {:?}",
            peer_reports.len(),
            total_size,
            peer_reports
        );
    }
}

impl<S> Future for SelfCheck<S>
where
    S: SharedSessionBook + Send + Unpin + 'static,
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
        self.as_ref().report_allowlist();

        Poll::Pending
    }
}
