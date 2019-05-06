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

pub trait JoinReactor<R>: Reactor {
    type JoinedOutput;

    fn join(self, down: R) -> Self::JoinedOutput;
}

pub trait ChainReactor<R>: Reactor {
    type ChainedOutput;

    fn chain(self, right: R) -> Self::ChainedOutput;
}

#[derive(Debug, Clone)]
pub enum ReactorMessage {
    Inbound(RecvMessage<PackedMessage>),
    Outbound(OutboundMessage),
}

pub enum Reaction<M> {
    Message(M),
    Done(FutReactorResult),
}

pub struct JoinedReactor<U, D> {
    upper: U,
    down:  D,
}

impl<M, U, D> JoinReactor<D> for U
where
    M: Clone,
    U: Reactor<Input = M, Output = Reaction<M>>,
    D: Reactor<Input = <U as Reactor>::Input, Output = <U as Reactor>::Output>,
{
    type JoinedOutput = JoinedReactor<U, D>;

    fn join(self, down: D) -> Self::JoinedOutput {
        JoinedReactor { upper: self, down }
    }
}

impl<M, U, D> Reactor for JoinedReactor<U, D>
where
    M: Clone,
    U: Reactor<Input = M, Output = Reaction<M>>,
    D: Reactor<Input = <U as Reactor>::Input, Output = <U as Reactor>::Output>,
{
    type Input = <U as Reactor>::Input;
    type Output = <U as Reactor>::Output;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        let upper_result = self.upper.react(broadcaster.clone(), input.clone());
        let down_result = self.down.react(broadcaster, input);

        match (upper_result, down_result) {
            (Reaction::Message(msg), Reaction::Message(_)) => Reaction::Message(msg),
            (Reaction::Done(upper_fut), Reaction::Done(down_fut)) => {
                Reaction::Done(Box::new(upper_fut.join(down_fut).map(|_| ())))
            }
            (Reaction::Done(fut), Reaction::Message(_))
            | (Reaction::Message(_), Reaction::Done(fut)) => Reaction::Done(Box::new(fut)),
        }
    }
}

pub struct ChainedReactor<L, R> {
    left:  L,
    right: R,
}

impl<M, L, R, T> ChainReactor<R> for L
where
    L: Reactor<Input = M, Output = T>,
    R: Reactor<Input = <L as Reactor>::Output, Output = Reaction<M>>,
{
    type ChainedOutput = ChainedReactor<L, R>;

    fn chain(self, right: R) -> Self::ChainedOutput {
        ChainedReactor { left: self, right }
    }
}

impl<M, L, R, T> Reactor for ChainedReactor<L, R>
where
    L: Reactor<Input = M, Output = T>,
    R: Reactor<Input = <L as Reactor>::Output, Output = Reaction<M>>,
{
    type Input = <L as Reactor>::Input;
    type Output = <R as Reactor>::Output;

    fn react(&mut self, broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        let left_ret = self.left.react(broadcaster.clone(), input);
        self.right.react(broadcaster.clone(), left_ret)
    }
}
