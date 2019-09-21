use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use protocol::{
    traits::{Context, MemPool, MemPoolAdapter, MessageHandler, Priority, Rpc},
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
    #[serde(with = "core_network::serde")]
    pub stx: SignedTransaction,
}

pub struct NewTxsHandler<M, MA> {
    mem_pool: Arc<M>,

    pin_ma: PhantomData<MA>,
}

impl<M, MA> NewTxsHandler<M, MA>
where
    M: MemPool<MA>,
    MA: MemPoolAdapter,
{
    pub fn new(mem_pool: Arc<M>) -> Self {
        NewTxsHandler {
            mem_pool,

            pin_ma: PhantomData,
        }
    }
}

#[async_trait]
impl<M, MA> MessageHandler for NewTxsHandler<M, MA>
where
    M: MemPool<MA> + 'static,
    MA: MemPoolAdapter + 'static,
{
    type Message = MsgNewTxs;

    async fn process(&self, ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        let ctx = ctx.mark_network_origin_new_txs();

        self.mem_pool.insert(ctx, msg.stx).await
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

pub struct PullTxsHandler<N, M, MA> {
    network:  N,
    mem_pool: Arc<M>,

    pin_ma: PhantomData<MA>,
}

impl<N, M, MA> PullTxsHandler<N, M, MA>
where
    N: Rpc + 'static,
    M: MemPool<MA> + 'static,
    MA: MemPoolAdapter + 'static,
{
    pub fn new(network: N, mem_pool: Arc<M>) -> Self {
        PullTxsHandler {
            network,
            mem_pool,

            pin_ma: PhantomData,
        }
    }
}

#[async_trait]
impl<N, M, MA> MessageHandler for PullTxsHandler<N, M, MA>
where
    N: Rpc + 'static,
    M: MemPool<MA> + 'static,
    MA: MemPoolAdapter + 'static,
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
