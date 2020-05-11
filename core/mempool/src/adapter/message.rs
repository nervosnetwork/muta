use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures::future::{try_join_all, TryFutureExt};
use protocol::{
    traits::{Context, MemPool, MessageHandler, Priority, Rpc, TrustFeedback},
    types::{Hash, SignedTransaction},
};
use serde_derive::{Deserialize, Serialize};

use crate::context::TxContext;

pub const END_GOSSIP_NEW_TXS: &str = "/gossip/mempool/new_txs";
pub const RPC_PULL_TXS: &str = "/rpc_call/mempool/pull_txs";
pub const RPC_RESP_PULL_TXS: &str = "/rpc_resp/mempool/pull_txs";
pub const RPC_RESP_PULL_TXS_SYNC: &str = "/rpc_resp/mempool/pull_txs_sync";

#[derive(Debug, Serialize, Deserialize)]
pub struct MsgNewTxs {
    #[serde(with = "core_network::serde_multi")]
    pub batch_stxs: Vec<SignedTransaction>,
}

pub struct NewTxsHandler<M> {
    mem_pool: Arc<M>,
}

impl<M> NewTxsHandler<M>
where
    M: MemPool,
{
    pub fn new(mem_pool: Arc<M>) -> Self {
        NewTxsHandler { mem_pool }
    }
}

#[async_trait]
impl<M> MessageHandler for NewTxsHandler<M>
where
    M: MemPool + 'static,
{
    type Message = MsgNewTxs;

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        let ctx = ctx.mark_network_origin_new_txs();

        let insert_stx = |stx| -> _ {
            let mem_pool = Arc::clone(&self.mem_pool);
            let ctx = ctx.clone();

            tokio::spawn(async move {
                let inst = Instant::now();
                common_apm::metrics::mempool::MEMPOOL_COUNTER_STATIC
                    .insert_tx_from_p2p
                    .inc();
                if let Err(_) = mem_pool.insert(ctx, stx).await {
                    common_apm::metrics::mempool::MEMPOOL_RESULT_COUNTER_STATIC
                        .insert_tx_from_p2p
                        .failure
                        .inc();
                }
                common_apm::metrics::mempool::MEMPOOL_RESULT_COUNTER_STATIC
                    .insert_tx_from_p2p
                    .success
                    .inc();
                common_apm::metrics::mempool::MEMPOOL_TIME_STATIC
                    .insert_tx_from_p2p
                    .observe(common_apm::metrics::duration_to_sec(inst.elapsed()));
            })
        };

        // Concurrently insert them
        if try_join_all(
            msg.batch_stxs
                .into_iter()
                .map(insert_stx)
                .collect::<Vec<_>>(),
        )
        .await
        .map(|_| ())
        .is_err()
        {
            log::error!("[core_mempool] mempool batch insert error");
        }

        TrustFeedback::Neutral
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MsgPullTxs {
    #[serde(with = "core_network::serde_multi")]
    pub hashes: Vec<Hash>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MsgPushTxs {
    #[serde(with = "core_network::serde_multi")]
    pub sig_txs: Vec<SignedTransaction>,
}

pub struct PullTxsHandler<N, M> {
    network:  Arc<N>,
    mem_pool: Arc<M>,
}

impl<N, M> PullTxsHandler<N, M>
where
    N: Rpc + 'static,
    M: MemPool + 'static,
{
    pub fn new(network: Arc<N>, mem_pool: Arc<M>) -> Self {
        PullTxsHandler { network, mem_pool }
    }
}

#[async_trait]
impl<N, M> MessageHandler for PullTxsHandler<N, M>
where
    N: Rpc + 'static,
    M: MemPool + 'static,
{
    type Message = MsgPullTxs;

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        let push_txs = async move {
            let ret = self
                .mem_pool
                .get_full_txs(ctx.clone(), msg.hashes)
                .await
                .map(|sig_txs| MsgPushTxs { sig_txs });

            self.network
                .response::<MsgPushTxs>(ctx, RPC_RESP_PULL_TXS, ret, Priority::High)
                .await
        };

        push_txs
            .unwrap_or_else(move |err| log::warn!("[core_mempool] push txs {}", err))
            .await;

        TrustFeedback::Neutral
    }
}
