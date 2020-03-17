use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{try_join_all, TryFutureExt};
use protocol::{
    traits::{Context, MemPool, MessageHandler, Priority, Rpc, Storage},
    types::{Hash, SignedTransaction},
};
use serde_derive::{Deserialize, Serialize};

use crate::context::TxContext;

pub const END_GOSSIP_NEW_TXS: &str = "/gossip/mempool/new_txs";
pub const RPC_PULL_TXS: &str = "/rpc_call/mempool/pull_txs";
pub const RPC_RESP_PULL_TXS: &str = "/rpc_resp/mempool/pull_txs";
pub const RPC_PULL_TXS_SYNC: &str = "/rpc_call/mempool/pull_txs_sync";
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

    async fn process(&self, ctx: Context, msg: Self::Message) {
        let ctx = ctx.mark_network_origin_new_txs();

        let insert_stx = |stx| -> _ {
            let mem_pool = Arc::clone(&self.mem_pool);
            let ctx = ctx.clone();

            tokio::spawn(async move { mem_pool.insert(ctx, stx).await })
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

    async fn process(&self, ctx: Context, msg: Self::Message) {
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
    }
}

pub struct PullTxsSyncHandler<N, M> {
    network: Arc<N>,
    storage: Arc<M>,
}

impl<N, M> PullTxsSyncHandler<N, M>
where
    N: Rpc + 'static,
    M: Storage + 'static,
{
    pub fn new(network: Arc<N>, storage: Arc<M>) -> Self {
        PullTxsSyncHandler { network, storage }
    }
}

#[async_trait]
impl<N, M> MessageHandler for PullTxsSyncHandler<N, M>
where
    N: Rpc + 'static,
    M: Storage + 'static,
{
    type Message = MsgPullTxs;

    async fn process(&self, ctx: Context, msg: Self::Message) {
        let futs = msg
            .hashes
            .into_iter()
            .map(|tx_hash| self.storage.get_transaction_by_hash(tx_hash))
            .collect::<Vec<_>>();
        let ret = try_join_all(futs)
            .await
            .map(|sig_txs| MsgPushTxs { sig_txs });

        self.network
            .response(ctx, RPC_RESP_PULL_TXS_SYNC, ret, Priority::High)
            .unwrap_or_else(move |e| log::warn!("[core_mempool] push txs {}", e))
            .await;
    }
}
