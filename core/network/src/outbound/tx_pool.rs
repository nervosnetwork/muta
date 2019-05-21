use futures::prelude::{FutureExt, TryFutureExt};

use core_runtime::{network::TransactionPool, FutRuntimeResult, TransactionPoolError};

use core_context::Context;
use core_network_message::Method;
use core_network_message::{common::PullTxs, tx_pool::BroadcastTxs};
use core_types::{Hash, SignedTransaction};

use crate::outbound::Mode;
use crate::{BytesBroadcaster, OutboundHandle};

impl TransactionPool for OutboundHandle {
    fn broadcast_batch(&self, txs: Vec<SignedTransaction>) {
        let outbound = self.clone();

        let job = async move {
            let data = BroadcastTxs::from(txs);

            // TODO: retry ?
            outbound.silent_broadcast(Method::BroadcastTxs, data, Mode::Normal);
        };

        tokio::run(job.unit_error().boxed().compat());
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
