use std::sync::Arc;

use async_trait::async_trait;
use futures::future::try_join_all;
use protocol::{
    traits::{Context, MemPool, MessageHandler, Priority, Rpc},
    types::{Hash, SignedTransaction},
    ProtocolResult,
};
use serde_derive::{Deserialize, Serialize};

use crate::context::TxContext;

pub const END_GOSSIP_NEW_TXS: &str = "/gossip/mempool/new_txs";
pub const END_RPC_PULL_TXS: &str = "/rpc_call/mempool/pull_txs";
pub const END_RESP_PULL_TXS: &str = "/rpc_resp/mempool/pull_txs";

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

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        let ctx = ctx.mark_network_origin_new_txs();

        let insert_stx = |stx| -> _ {
            let mem_pool = Arc::clone(&self.mem_pool);
            let ctx = ctx.clone();

            runtime::spawn(async move { mem_pool.insert(ctx, stx).await })
        };

        // Concurrently insert them
        try_join_all(
            msg.batch_stxs
                .into_iter()
                .map(insert_stx)
                .collect::<Vec<_>>(),
        )
        .await
        .map(|_| ())
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
    network:  N,
    mem_pool: Arc<M>,
}

impl<N, M> PullTxsHandler<N, M>
where
    N: Rpc + 'static,
    M: MemPool + 'static,
{
    pub fn new(network: N, mem_pool: Arc<M>) -> Self {
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

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        let sig_txs = self.mem_pool.get_full_txs(ctx.clone(), msg.hashes).await?;
        let resp_msg = MsgPushTxs { sig_txs };

        self.network
            .response::<MsgPushTxs>(ctx, END_RESP_PULL_TXS, resp_msg, Priority::High)
            .await
    }
}
