use futures::prelude::{FutureExt, TryFutureExt};

use core_context::Context;
use core_network_message::common::PullTxs;
use core_network_message::sync::{BroadcastStatus, PullBlocks};
use core_network_message::Method;
use core_runtime::network::{FutSyncResult, Synchronizer};
use core_runtime::{SyncStatus, SynchronizerError};
use core_types::{Block, Hash, SignedTransaction};

use crate::outbound::Mode;
use crate::{BytesBroadcaster, OutboundHandle};

impl Synchronizer for OutboundHandle {
    fn broadcast_status(&self, status: SyncStatus) {
        let outbound = self.clone();

        let job = async move {
            let data = BroadcastStatus::from(status.hash, status.height);

            outbound.silent_broadcast(Method::SyncBroadcastStatus, data, Mode::Normal);
        };

        tokio::spawn(job.unit_error().boxed().compat());
    }

    fn pull_blocks(&self, ctx: Context, heights: Vec<u64>) -> FutSyncResult<Vec<Block>> {
        let outbound = self.clone();

        callback_broadcast!(
            outbound,
            ctx,
            heights,
            PullBlocks,
            Method::SyncPullBlocks,
            SynchronizerError::Internal
        )
    }

    // TODO: Use FutureObj, so that we don't need "to_vec()"
    fn pull_txs_sync(
        &self,
        ctx: Context,
        tx_hashes: &[Hash],
    ) -> FutSyncResult<Vec<SignedTransaction>> {
        let outbound = self.clone();
        let tx_hashes = tx_hashes.to_vec();

        callback_broadcast!(
            outbound,
            ctx,
            tx_hashes,
            PullTxs,
            Method::SyncPullTxs,
            SynchronizerError::Internal
        )
    }
}
