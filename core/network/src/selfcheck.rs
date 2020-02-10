use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures::task::AtomicWaker;
use log::info;

use crate::{common::HeartBeat, traits::PeerQuerier};

pub struct SelfCheckConfig {
    pub interval: Duration,
}

pub(crate) struct SelfCheck<PQ> {
    peer_querier: PQ,
    heart_beat:   Option<HeartBeat>,
    hb_waker:     Arc<AtomicWaker>,
}

impl<PQ> SelfCheck<PQ>
where
    PQ: PeerQuerier + Send + Unpin + 'static,
{
    pub fn new(peer_querier: PQ, config: SelfCheckConfig) -> Self {
        let waker = Arc::new(AtomicWaker::new());
        let heart_beat = HeartBeat::new(Arc::clone(&waker), config.interval);

        SelfCheck {
            peer_querier,
            heart_beat: Some(heart_beat),
            hb_waker: waker,
        }
    }

    fn report_pending_data(&self) {
        let peers = self.peer_querier.connected_peers();
        let mut total_size = 0;

        let peer_report = peers
            .into_iter()
            .map(|peer| {
                let connected_addr = self.peer_querier.connected_addr(&peer);
                let data_size = self.peer_querier.pending_data_size(&peer) / (1000 * 1000); // MB not MiB

                total_size += data_size;
                (connected_addr, data_size)
            })
            .collect::<Vec<_>>();

        info!(
            "total pending size {} MB, peer(s) {:?}",
            total_size, peer_report
        );
    }
}

impl<PQ> Future for SelfCheck<PQ>
where
    PQ: PeerQuerier + Send + Unpin + 'static,
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
