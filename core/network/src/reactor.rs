pub mod inbound;
pub mod outbound;
pub use inbound::InboundReactor;
pub use outbound::{OutboundMessage, OutboundReactor};

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use futures::prelude::Future;
use futures::sync::mpsc::Sender;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::p2p::{Broadcaster, PackedMessage, RecvMessage};

pub type FutReactorResult = Box<dyn Future<Item = (), Error = ()> + Send + 'static>;
// TODO: remove lock
pub type CallbackMap = Arc<RwLock<HashMap<Uuid, Sender<Box<dyn Any + Send + 'static>>>>>;

pub trait Reactor {
    type Input;
    type Output;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output;
}

impl<M, R> Reactor for Box<R>
where
    R: Reactor<Input = M, Output = Reaction<M>>,
{
    type Input = <R as Reactor>::Input;
    type Output = <R as Reactor>::Output;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        self.as_mut().react(broadcaster, input)
    }
}

pub trait ChainReactor<R>: Reactor {
    type ChainedOutput;

    fn chain(self, right: R) -> Self::ChainedOutput;
}

#[derive(Debug)]
pub enum ReactorMessage {
    Inbound(RecvMessage<PackedMessage>),
    Outbound(OutboundMessage),
}

pub enum Reaction<M> {
    Message(M),
    Done(FutReactorResult),
}

pub struct ChainedReactor<L, R> {
    left:  L,
    right: R,
}

impl<M, L, R> ChainReactor<R> for L
where
    L: Reactor<Input = M, Output = Reaction<M>>,
    R: Reactor<Input = <L as Reactor>::Input, Output = <L as Reactor>::Output>,
{
    type ChainedOutput = ChainedReactor<L, R>;

    fn chain(self, right: R) -> Self::ChainedOutput {
        ChainedReactor { left: self, right }
    }
}

impl<M, L, R> Reactor for ChainedReactor<L, R>
where
    L: Reactor<Input = M, Output = Reaction<M>>,
    R: Reactor<Input = <L as Reactor>::Input, Output = <L as Reactor>::Output>,
{
    type Input = <L as Reactor>::Input;
    type Output = <L as Reactor>::Output;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        match self.left.react(broadcaster.clone(), input) {
            Reaction::Message(msg) => self.right.react(broadcaster.clone(), msg),
            Reaction::Done(done) => Reaction::Done(done),
        }
    }
}
