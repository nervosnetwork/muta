use futures::prelude::TryFutureExt;

use core_context::Context;
use core_network_message::Method;
use core_network_message::{common::PullTxs, tx_pool::BroadcastTxs};
use core_runtime::{network::TransactionPool, FutTxPoolResult, TransactionPoolError};
use core_types::{Hash, SignedTransaction};

use crate::callback_map::CallId;
use crate::outbound::{BytesBroadcaster, CallbackChannel, Mode, CALL_ID_KEY};
use crate::OutboundHandle;

impl<B, C> TransactionPool for OutboundHandle<B, C>
where
    B: BytesBroadcaster + Clone + Send + Sync + 'static,
    C: CallbackChannel + Send + Sync + 'static,
{
    fn broadcast_batch(&self, txs: Vec<SignedTransaction>) {
        let data = BroadcastTxs::from(txs);

        // TODO: retry ?
        self.silent_broadcast(Method::BroadcastTxs, data, Mode::Normal);
    }

    fn pull_txs(&self, ctx: Context, hashes: Vec<Hash>) -> FutTxPoolResult<Vec<SignedTransaction>> {
        let outbound = self.clone();

        let call_id = self.cb_chan.new_call_id();
        let ctx = ctx.with_value::<CallId>(CALL_ID_KEY, call_id);
        let data = PullTxs::from(call_id.value(), hashes);

        let fut = outbound.rpc(ctx, Method::PullTxs, data);
        Box::pin(fut.map_err(TransactionPoolError::Internal))
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use core_context::{Context, P2P_SESSION_ID};
    use core_network_message::{common::PullTxs, tx_pool::BroadcastTxs, Method};
    use core_runtime::{network::TransactionPool, TransactionPoolError};
    use core_types::{Hash, SignedTransaction};

    use crate::outbound::tests::{encode_bytes, new_outbound};
    use crate::outbound::Mode;
    use crate::p2p::{Scope, SessionId};

    #[test]
    fn test_broadcast_batch() {
        let stxs = vec![SignedTransaction::default(), SignedTransaction::default()];
        let bytes = encode_bytes(&BroadcastTxs::from(stxs.clone()), Method::BroadcastTxs);

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcast_batch(stxs);

        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Normal, Scope::All, bytes))
        );
    }

    #[test]
    fn test_broadcast_batch_but_fail() {
        let stxs = vec![SignedTransaction::default(), SignedTransaction::default()];
        let (outbound, _) = new_outbound::<()>();

        outbound.broadcaster.reply_err(true);
        outbound.broadcast_batch(stxs);

        assert_eq!(outbound.broadcaster.broadcasted_bytes(), None);
    }

    #[test]
    fn test_pull_txs() {
        let hashes = vec![Hash::default(), Hash::default()];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);
        let bytes = encode_bytes(&PullTxs::from(1, hashes.clone()), Method::PullTxs);
        let scope = Scope::Single(SessionId::new(1));

        let expect_resp = vec![SignedTransaction::default(), SignedTransaction::default()];
        let (outbound, done_tx) = new_outbound::<Vec<SignedTransaction>>();
        done_tx.try_send(expect_resp.clone()).unwrap();

        let resp = block_on(outbound.pull_txs(ctx, hashes)).unwrap();
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
        match block_on(outbound.pull_txs(ctx, hashes)) {
            Err(TransactionPoolError::Internal(str)) => {
                assert!(str.contains("session id not found"))
            }
            _ => panic!("should return TransactionPoolError::Internal"),
        }
    }

    #[test]
    fn test_pull_txs_but_broadcast_fail() {
        let hashes = vec![Hash::default()];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcaster.reply_err(true);

        match block_on(outbound.pull_txs(ctx, hashes)) {
            Err(TransactionPoolError::Internal(_)) => (),
            _ => panic!("should return TransactionPoolError::Internal"),
        }
    }

    #[test]
    fn test_pull_txs_with_disconnected_done_tx() {
        let hashes = vec![Hash::default()];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);

        let (outbound, done_tx) = new_outbound::<Vec<SignedTransaction>>();
        drop(done_tx);

        match block_on(outbound.pull_txs(ctx, hashes)) {
            Err(TransactionPoolError::Internal(str)) => {
                assert!(str.contains("done_rx return None"))
            }
            _ => panic!("should return TransactionPoolError::Internal"),
        }
    }

    #[test]
    fn test_pull_txs_timeout() {
        let hashes = vec![Hash::default()];
        let ctx = Context::new().with_value(P2P_SESSION_ID, 1usize);
        let bytes = encode_bytes(&PullTxs::from(1, hashes.clone()), Method::PullTxs);
        let scope = Scope::Single(SessionId::new(1));

        let (outbound, _done_tx) = new_outbound::<Vec<SignedTransaction>>();

        match block_on(outbound.pull_txs(ctx, hashes)) {
            Err(TransactionPoolError::Internal(str)) => assert!(str.contains("timeout")),
            _ => panic!("should return TransactionPoolError::Internal indicates timeout"),
        }
        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, scope, bytes))
        );
    }
}
