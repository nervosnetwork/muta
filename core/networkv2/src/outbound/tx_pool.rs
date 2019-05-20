use futures::prelude::TryFutureExt;

use core_runtime::{network::TransactionPool, FutRuntimeResult, TransactionPoolError};

use core_context::Context;
use core_networkv2_message::Method;
use core_networkv2_message::{common::PullTxs, tx_pool::BroadcastTxs};
use core_types::{Hash, SignedTransaction};

use crate::outbound::Mode;
use crate::{BytesBroadcaster, OutboundHandle};

impl TransactionPool for OutboundHandle {
    fn broadcast_batch(&self, txs: Vec<SignedTransaction>) {
        let data = BroadcastTxs::from(txs);

        // TODO: retry ?
        self.silent_broadcast(Method::BroadcastTxs, data, Mode::Normal);
    }

    fn pull_txs(
        &self,
        ctx: Context,
        hashes: Vec<Hash>,
    ) -> FutRuntimeResult<Vec<SignedTransaction>, TransactionPoolError> {
        let outbound = self.clone();

        callback_broadcast!(
            outbound,
            ctx,
            hashes,
            PullTxs,
            Method::PullTxs,
            TransactionPoolError::Internal
        )
    }
}
