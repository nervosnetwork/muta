pub mod consensus;
pub mod sync;
pub mod tx_pool;
pub use consensus::ConsensusReactor;
pub use sync::SyncReactor;
pub use tx_pool::TxPoolReactor;

use std::io::{self as io, ErrorKind};
use std::pin::Pin;
use std::sync::Arc;

use futures::future::{ready, FutureObj};
use futures::prelude::{FutureExt, Stream, TryFutureExt};
use futures::task::{Context as FutTaskContext, Poll};
use log::{error, info};

use common_channel::Receiver;
use core_context::P2P_SESSION_ID;
use core_networkv2_message::{Codec, Message, Method};
use core_runtime::{Consensus, Storage, TransactionPool};

use crate::p2p::SessionMessage;
use crate::{CallbackMap, Error, OutboundHandle};

pub struct InboundHandle<T, C, S>
where
    T: TransactionPool + 'static,
    C: Consensus + 'static,
    S: Storage + 'static,
{
    inbound: Receiver<SessionMessage>,

    tx_pool:   TxPoolReactor<T>,
    consensus: ConsensusReactor<C>,
    sync:      SyncReactor<C, S>,
}

impl<T, C, S> InboundHandle<T, C, S>
where
    T: TransactionPool + 'static,
    C: Consensus + 'static,
    S: Storage + 'static,
{
    pub fn new(
        callback: Arc<CallbackMap>,
        inbound: Receiver<SessionMessage>,
        outbound: OutboundHandle,
        tx_pool: Arc<T>,
        consensus: Arc<C>,
        storage: Arc<S>,
    ) -> Self {
        let tx_pool_reactor = TxPoolReactor::new(
            outbound.clone(),
            Arc::clone(&callback),
            Arc::clone(&tx_pool),
        );
        let consensus_reactor = ConsensusReactor::new(Arc::clone(&consensus));
        let sync_reactor = SyncReactor::new(
            Arc::clone(&consensus),
            Arc::clone(&storage),
            Arc::clone(&callback),
            outbound.clone(),
        );

        InboundHandle {
            inbound,

            tx_pool: tx_pool_reactor,
            consensus: consensus_reactor,
            sync: sync_reactor,
        }
    }

    fn handle_inbound_msg(&self, session_msg: SessionMessage) -> FutureObj<'static, ()> {
        let SessionMessage { id, addr, body } = session_msg;
        let ctx = core_context::Context::new().with_value(P2P_SESSION_ID, id.value());
        let data = body;

        let tx_pool = self.tx_pool.clone();
        let consensus = self.consensus.clone();
        let sync = self.sync.clone();

        let job = async move {
            // TODO: report error upstream
            let Message {
                method,
                data,
                data_size,
            } = Message::decode(&data)?;

            if data_size != data.len() as u64 {
                Err(Error::IoError(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "net [inbound]: data corruption",
                )))?;
            }

            let method = Method::from_u32(method)?;
            match method {
                Method::PullTxs | Method::PushTxs | Method::BroadcastTxs => {
                    await!(tx_pool.react(ctx, method, data.to_vec()))?
                }

                Method::Proposal | Method::Vote => {
                    await!(consensus.react(ctx, method, data.to_vec()))?
                }

                Method::SyncBroadcastStatus
                | Method::SyncPullBlocks
                | Method::SyncPushBlocks
                | Method::SyncPullTxs
                | Method::SyncPushTxs => await!(sync.react(ctx, method, data.to_vec()))?,
            }

            Ok(())
        };

        let job = job.then(move |ret: Result<(), Error>| {
            if let Err(err) = ret {
                error!("net [inbound]: [addr: {}, err: {:?}]", addr, err);
            }

            ready(())
        });

        FutureObj::new(Box::new(job))
    }
}

impl<T, C, S> Stream for InboundHandle<T, C, S>
where
    T: TransactionPool + 'static,
    C: Consensus + 'static,
    S: Storage + 'static,
{
    type Item = ();

    fn poll_next(
        mut self: Pin<&mut Self>,
        ctx: &mut FutTaskContext<'_>,
    ) -> Poll<Option<Self::Item>> {
        match Stream::poll_next(Pin::new(&mut self.inbound), ctx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                // TODO: check network state, shutdown or unexpected stop?
                // TODO: if shutdown, then do clean up
                // TODO: if unexpected stop, then try to restart
                info!("net [inbound]: stop");
                Poll::Ready(None)
            }
            Poll::Ready(Some(session_msg)) => {
                let job = self.handle_inbound_msg(session_msg);
                tokio::spawn(job.unit_error().boxed().compat());

                Poll::Ready(Some(()))
            }
        }
    }
}
