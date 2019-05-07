pub mod consensus;
pub mod tx_pool;
use tx_pool::TransactionPoolReactor;
pub mod synchronizer;
use synchronizer::SynchronizerReactor;

use std::sync::Arc;

use log::debug;

use consensus::ConsensusReactor;
use core_consensus::{Consensus, Engine, Synchronizer};
use core_context::{Context, P2P_SESSION_ID};
use core_crypto::Crypto;
use core_runtime::{Executor, TransactionPool};
use core_storage::Storage;

use crate::p2p::{Broadcaster, Message, PackedMessage};
use crate::reactor::{CallbackMap, Reaction, Reactor, ReactorMessage};

// TODO: allow plugable chained reactors from components
pub struct InboundReactor<E, T, S, C, Y, Con>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    Y: Synchronizer + 'static,
{
    tx_pool:      Arc<T>,
    storage:      Arc<S>,
    engine:       Arc<Engine<E, T, S, C>>,
    synchronizer: Arc<Y>,
    consensus:    Arc<Con>,
    callback_map: CallbackMap,
}

impl<E, T, S, C, Y, Con> InboundReactor<E, T, S, C, Y, Con>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    Y: Synchronizer + 'static,
    Con: Consensus + 'static,
{
    pub fn new(
        tx_pool: Arc<T>,
        storage: Arc<S>,
        engine: Arc<Engine<E, T, S, C>>,
        synchronizer: Arc<Y>,
        consensus: Arc<Con>,
        callback_map: CallbackMap,
    ) -> Self {
        InboundReactor {
            tx_pool,
            engine,
            storage,
            synchronizer,
            consensus,
            callback_map,
        }
    }
}

impl<E, T, S, C, Y, Con> Reactor for InboundReactor<E, T, S, C, Y, Con>
where
    E: Executor + 'static,
    T: TransactionPool + 'static,
    S: Storage + 'static,
    C: Crypto + 'static,
    Y: Synchronizer + 'static,
    Con: Consensus + 'static,
{
    type Input = ReactorMessage;
    type Output = Reaction<ReactorMessage>;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        match input {
            ReactorMessage::Inbound(recv_msg) => {
                let session_ctx = Context::new()
                    .with_value::<usize>(P2P_SESSION_ID, recv_msg.session_id().value());
                let tx_pool = Arc::clone(&self.tx_pool);
                let consensus = Arc::clone(&self.consensus);
                let callback = Arc::clone(&self.callback_map);

                if let PackedMessage { message: Some(msg) } = recv_msg.take_msg() {
                    match msg {
                        Message::TxPoolMessage(msg) => {
                            let mut tx_pool_reactor =
                                TransactionPoolReactor::new(tx_pool, callback);
                            tx_pool_reactor.react(broadcaster, (session_ctx.clone(), msg))
                        }
                        Message::SynchronizerMessage(msg) => {
                            let mut synchronizer_reactor = SynchronizerReactor::new(
                                Arc::clone(&self.storage),
                                Arc::clone(&self.engine),
                                Arc::clone(&self.synchronizer),
                                Arc::clone(&self.consensus),
                                callback,
                            );
                            synchronizer_reactor.react(broadcaster, (session_ctx.clone(), msg))
                        }
                        Message::ConsensusMessage(msg) => {
                            let mut consensus_reactor = ConsensusReactor::new(consensus);
                            consensus_reactor.react(broadcaster, (session_ctx.clone(), msg))
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
