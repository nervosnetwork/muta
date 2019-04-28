pub mod tx_pool;
use tx_pool::TransactionPoolReactor;

use std::sync::Arc;

use log::debug;

use core_context::{Context, P2P_SESSION_ID};
use core_runtime::TransactionPool;

use crate::p2p::{Broadcaster, Message, PackedMessage};
use crate::reactor::{CallbackMap, Reaction, Reactor, ReactorMessage};

// TODO: allow plugable chained reactors from components
pub struct InboundReactor<T> {
    tx_pool: Arc<T>,

    callback_map: CallbackMap,
}

impl<T> InboundReactor<T>
where
    T: TransactionPool + 'static,
{
    pub fn new(tx_pool: Arc<T>, callback_map: CallbackMap) -> Self {
        InboundReactor {
            tx_pool,

            callback_map,
        }
    }
}

impl<T> Reactor for InboundReactor<T>
where
    T: TransactionPool + 'static,
{
    type Input = ReactorMessage;
    type Output = Reaction<ReactorMessage>;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        match input {
            ReactorMessage::Inbound(recv_msg) => {
                let session_ctx = Context::new()
                    .with_value::<usize>(P2P_SESSION_ID, recv_msg.session_id().value());
                let tx_pool = Arc::clone(&self.tx_pool);
                let callback = Arc::clone(&self.callback_map);

                if let PackedMessage { message: Some(msg) } = recv_msg.take_msg() {
                    match msg {
                        Message::TxPoolMessage(msg) => {
                            let mut tx_pool_reactor =
                                TransactionPoolReactor::new(tx_pool, callback);
                            tx_pool_reactor.react(broadcaster, (session_ctx.clone(), msg))
                        }
                    }
                } else {
                    unreachable!()
                }
            }
            msg => Reaction::Message(msg),
        }
    }
}

pub struct LoggerInboundReactor;

impl Reactor for LoggerInboundReactor {
    type Input = ReactorMessage;
    type Output = Reaction<ReactorMessage>;

    fn react(&mut self, _broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        match input {
            ReactorMessage::Inbound(recv_msg) => {
                debug!("inbound: recv msg: {:?}", recv_msg);

                Reaction::Message(ReactorMessage::Inbound(recv_msg))
            }
            msg => Reaction::Message(msg),
        }
    }
}
