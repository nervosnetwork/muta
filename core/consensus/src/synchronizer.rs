use std::sync::Arc;
use std::time::Duration;

use futures::compat::Stream01CompatExt;
use futures::future;
use futures::prelude::StreamExt;
use futures_timer::Interval;
use log;
use old_futures::{self, stream::Stream, Future as OldFuture};

use core_context::Context;
use core_pubsub::channel::pubsub::Receiver;
use core_runtime::{network::Synchronizer as Network, Storage, SyncStatus};
use core_types::Block;

pub struct SynchronizerManager<S, Sy> {
    broadcast_status_interval: u64,
    synchronizer:              Arc<Sy>,
    storage:                   Arc<S>,
}

impl<S, Sy> SynchronizerManager<S, Sy>
where
    S: Storage + 'static,
    Sy: Network + 'static,
{
    pub fn new(synchronizer: Arc<Sy>, storage: Arc<S>, broadcast_status_interval: u64) -> Self {
        Self {
            broadcast_status_interval,
            synchronizer,
            storage,
        }
    }

    pub fn start(&self, mut sub_block: Receiver<Block>) {
        let synchronizer = Arc::clone(&self.synchronizer);
        let storage = Arc::clone(&self.storage);

        let interval_broadcaster =
            Interval::new(Duration::from_millis(self.broadcast_status_interval))
                .map_err(|e| log::error!("interval err: {:?}", e))
                .and_then(move |_| {
                    storage
                        .get_latest_block(Context::new())
                        .map_err(|e| log::error!("get_latest_block err: {:?}", e))
                })
                .compat()
                .map(std::result::Result::ok);

        rayon::spawn(move || {
            futures::executor::block_on(
                futures::stream::select(sub_block.boxed(), interval_broadcaster.boxed())
                    .filter_map(future::ready)
                    .for_each(move |block| {
                        let status = SyncStatus {
                            hash:   block.hash,
                            height: block.header.height,
                        };
                        log::debug!("broadcast status: {:?}", &status);
                        synchronizer.broadcast_status(status);
                        future::ready(())
                    }),
            );
        });
    }
}
