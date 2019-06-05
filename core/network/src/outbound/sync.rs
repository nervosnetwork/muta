use futures::prelude::TryFutureExt;

use core_context::Context;
use core_network_message::common::PullTxs;
use core_network_message::sync::{BroadcastStatus, PullBlocks};
use core_network_message::Method;
use core_runtime::network::{FutSyncResult, Synchronizer};
use core_runtime::{SyncStatus, SynchronizerError};
use core_types::{Block, Hash, SignedTransaction};

use crate::callback_map::CallId;
use crate::outbound::{BytesBroadcaster, CallbackChannel, Mode, CALL_ID_KEY};
use crate::OutboundHandle;

impl<B, C> Synchronizer for OutboundHandle<B, C>
where
    B: BytesBroadcaster + Clone + Send + Sync + 'static,
    C: CallbackChannel + Send + Sync + 'static,
{
    fn broadcast_status(&self, status: SyncStatus) {
        let data = BroadcastStatus::from(status.hash, status.height);

        self.silent_broadcast(Method::SyncBroadcastStatus, data, Mode::Normal);
    }

    fn pull_blocks(&self, ctx: Context, heights: Vec<u64>) -> FutSyncResult<Vec<Block>> {
        let outbound = self.clone();

        let call_id = self.cb_chan.new_call_id();
        let ctx = ctx.with_value::<CallId>(CALL_ID_KEY, call_id);
        let data = PullBlocks::from(call_id.value(), heights);

        let fut = outbound.rpc(ctx, Method::SyncPullBlocks, data);
        Box::pin(fut.map_err(SynchronizerError::Internal))
    }

    fn pull_txs(&self, ctx: Context, hashes: &[Hash]) -> FutSyncResult<Vec<SignedTransaction>> {
        let outbound = self.clone();

        let call_id = self.cb_chan.new_call_id();
        let ctx = ctx.with_value::<CallId>(CALL_ID_KEY, call_id);
        let data = PullTxs::from(call_id.value(), hashes.to_vec());

        let fut = outbound.rpc(ctx, Method::SyncPullTxs, data);
        Box::pin(fut.map_err(SynchronizerError::Internal))
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use core_context::{Context, P2P_SESSION_ID};
    use core_network_message::sync::{BroadcastStatus, PullBlocks};
    use core_network_message::{common::PullTxs, Method};
    use core_runtime::{network::Synchronizer, SyncStatus, SynchronizerError};
    use core_types::{Block, Hash, SignedTransaction};

    use crate::outbound::tests::{encode_bytes, new_outbound};
    use crate::outbound::Mode;
    use crate::p2p::{Scope, SessionId};

    #[test]
    fn test_broadcast_status() {
        let status = SyncStatus {
            hash:   Hash::default(),
            height: 2020,
        };
        let bytes = encode_bytes(
            &BroadcastStatus::from(Hash::default(), 2020),
            Method::SyncBroadcastStatus,
        );

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcast_status(status);

        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Normal, Scope::All, bytes))
        );
    }

    #[test]
    fn test_broadcast_status_but_fail() {
        let status = SyncStatus {
            hash:   Hash::default(),
            height: 2020,
        };

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcaster.reply_err(true);
        outbound.broadcast_status(status);

        assert_eq!(outbound.broadcaster.broadcasted_bytes(), None);
    }

    #[test]
    fn test_pull_blocks() {
        let heights = vec![2020, 2021];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);
        let bytes = encode_bytes(
            &PullBlocks::from(1, heights.clone()),
            Method::SyncPullBlocks,
        );
        let scope = Scope::Single(SessionId::new(1usize));

        let expect_resp = {
            let mut blk_2020 = Block::default();
            blk_2020.header.height = 2020;
            vec![blk_2020]
        };
        let (outbound, done_tx) = new_outbound::<Vec<Block>>();
        done_tx.try_send(expect_resp.clone()).unwrap();

        let resp = block_on(outbound.pull_blocks(ctx, heights)).unwrap();
        assert_eq!(resp.first().unwrap().header.height, 2020);
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, scope, bytes))
        );
    }

    #[test]
    fn test_pull_blocks_without_session_id() {
        let heights = vec![2099];
        let ctx = Context::new();

        let (outbound, _) = new_outbound::<()>();
        match block_on(outbound.pull_blocks(ctx, heights)) {
            Err(SynchronizerError::Internal(str)) => assert!(str.contains("session id not found")),
            _ => panic!("should return SynchronizerError::Internal"),
        }
    }

    #[test]
    fn test_pull_blocks_but_broadcast_fail() {
        let heights = vec![2099];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcaster.reply_err(true);

        match block_on(outbound.pull_blocks(ctx, heights)) {
            Err(SynchronizerError::Internal(_)) => (),
            _ => panic!("should return SynchronizerError::Internal"),
        }
    }

    #[test]
    fn test_pull_blocks_with_disconnected_done_tx() {
        let heights = vec![2099];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);

        let (outbound, done_tx) = new_outbound::<Vec<Block>>();
        drop(done_tx);

        match block_on(outbound.pull_blocks(ctx, heights)) {
            Err(SynchronizerError::Internal(str)) => assert!(str.contains("done_rx return None")),
            _ => panic!("should return SynchronizerError::Internal"),
        }
    }

    #[test]
    fn test_pull_blocks_timeout() {
        let heights = vec![2077];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);
        let bytes = encode_bytes(
            &PullBlocks::from(1, heights.clone()),
            Method::SyncPullBlocks,
        );
        let scope = Scope::Single(SessionId::new(1usize));

        let (outbound, _done_tx) = new_outbound::<Vec<Block>>();

        match block_on(outbound.pull_blocks(ctx, heights)) {
            Err(SynchronizerError::Internal(str)) => assert!(str.contains("timeout")),
            _ => panic!("should return SynchronizerError::Internal indicates timeout"),
        }
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, scope, bytes))
        );
    }

    #[test]
    fn test_pull_txs() {
        let hashes = vec![Hash::default(), Hash::default()];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);
        let bytes = encode_bytes(&PullTxs::from(1, hashes.clone()), Method::SyncPullTxs);
        let scope = Scope::Single(SessionId::new(1usize));

        let expect_resp = vec![SignedTransaction::default()];
        let (outbound, done_tx) = new_outbound::<Vec<SignedTransaction>>();
        done_tx.try_send(expect_resp.clone()).unwrap();

        let resp = block_on(outbound.pull_txs(ctx, hashes.as_slice())).unwrap();
        assert_eq!(resp, expect_resp);
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, scope, bytes))
        );
    }

    #[test]
    fn test_pull_txs_without_session_id() {
        let hashes = vec![Hash::default()];
        let ctx = Context::new();

        let (outbound, _) = new_outbound::<()>();
        match block_on(outbound.pull_txs(ctx, hashes.as_slice())) {
            Err(SynchronizerError::Internal(str)) => assert!(str.contains("session id not found")),
            _ => panic!("should return SynchronizerError::Internal"),
        }
    }

    #[test]
    fn test_pull_txs_but_broadcast_fail() {
        let hashes = vec![Hash::default()];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcaster.reply_err(true);

        match block_on(outbound.pull_txs(ctx, hashes.as_slice())) {
            Err(SynchronizerError::Internal(_)) => (),
            _ => panic!("should return SynchronizerError::Internal"),
        }
    }

    #[test]
    fn test_pull_txs_with_disconnected_done_tx() {
        let hashes = vec![Hash::default()];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);

        let (outbound, done_tx) = new_outbound::<Vec<SignedTransaction>>();
        drop(done_tx);

        match block_on(outbound.pull_txs(ctx, hashes.as_slice())) {
            Err(SynchronizerError::Internal(str)) => assert!(str.contains("done_rx return None")),
            _ => panic!("should return SynchronizerError::Internal"),
        }
    }

    #[test]
    fn test_pull_txs_timeout() {
        let hashes = vec![Hash::default()];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);
        let bytes = encode_bytes(&PullTxs::from(1, hashes.clone()), Method::SyncPullTxs);
        let scope = Scope::Single(SessionId::new(1usize));

        let (outbound, _done_tx) = new_outbound::<Vec<SignedTransaction>>();

        match block_on(outbound.pull_txs(ctx, hashes.as_slice())) {
            Err(SynchronizerError::Internal(str)) => assert!(str.contains("timeout")),
            _ => panic!("should return SynchronizerError::Internal indicates timeout"),
        }
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, scope, bytes))
        );
    }
}
