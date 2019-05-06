mod consensus;
mod tx_pool;

use consensus::{ConsensusMessage, ConsensusReactor};
use tx_pool::{TransactionPoolMessage, TransactionPoolReactor};

use std::sync::Arc;

use futures::sync::mpsc;

use crate::p2p::Broadcaster;
use crate::reactor::{CallbackMap, Reaction, Reactor, ReactorMessage};

#[derive(Debug, Clone)]
pub enum OutboundMessage {
    TransactionPool(TransactionPoolMessage),
    Consensus(ConsensusMessage),
}

#[derive(Clone)]
pub struct Sender {
    inner: mpsc::Sender<OutboundMessage>,
}

impl Sender {
    pub fn new(tx: mpsc::Sender<OutboundMessage>) -> Self {
        Sender { inner: tx }
    }

    pub fn try_send(
        &mut self,
        msg: OutboundMessage,
    ) -> Result<(), mpsc::TrySendError<OutboundMessage>> {
        self.inner.try_send(msg)
    }
}

// TODO: allow chained reactors from components
pub struct OutboundReactor {
    callback_map: CallbackMap,
}

impl OutboundReactor {
    pub fn new(callback_map: CallbackMap) -> Self {
        OutboundReactor { callback_map }
    }
}

impl Reactor for OutboundReactor {
    type Input = ReactorMessage;
    type Output = Reaction<ReactorMessage>;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        match input {
            ReactorMessage::Outbound(OutboundMessage::TransactionPool(tx_msg)) => {
                let mut tx_react = TransactionPoolReactor::new(Arc::clone(&self.callback_map));
                Reaction::Done(tx_react.react(broadcaster, tx_msg))
            }
            ReactorMessage::Outbound(OutboundMessage::Consensus(consensus_msg)) => {
                let mut consensus_react = ConsensusReactor::new();
                Reaction::Done(consensus_react.react(broadcaster, consensus_msg))
            }
            msg => Reaction::Message(msg),
        }
    }
}
