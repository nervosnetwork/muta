use futures::prelude::TryFutureExt;

use core_context::Context;
use core_networkv2_message::Method;
use core_networkv2_message::{
    common::PullTxs,
    sync::{BroadcastStatus, PullBlocks},
};
use core_runtime::network::{FutSyncResult, Synchronizer};
use core_runtime::{SyncStatus, SynchronizerError};
use core_types::{Block, Hash, SignedTransaction};

use crate::outbound::Mode;
use crate::{BytesBroadcaster, OutboundHandle};

impl Synchronizer for OutboundHandle {
    fn broadcast_status(&self, status: SyncStatus) {
        let data = BroadcastStatus::from(status.hash, status.height);

        self.silent_broadcast(Method::SyncBroadcastStatus, data, Mode::Normal);
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

    fn pull_txs_sync(
        &self,
        ctx: Context,
        tx_hashes: &[Hash],
    ) -> FutSyncResult<Vec<SignedTransaction>> {
        let outbound = self.clone();
        let tx_hashes = tx_hashes.iter().map(ToOwned::to_owned).collect::<Vec<_>>();

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
