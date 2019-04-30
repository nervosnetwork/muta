use std::sync::Arc;

use futures::future::Future;

use core_consensus::{Consensus, ConsensusError};
use core_context::Context;

use crate::p2p::message::consensus::{packed_message, ConsensusMessage, Proposal, Vote};
use crate::p2p::Broadcaster;
use crate::reactor::{Reaction, Reactor, ReactorMessage};

#[derive(Debug)]
enum Error {
    Consensus(ConsensusError),
}
impl From<ConsensusError> for Error {
    fn from(err: ConsensusError) -> Self {
        Error::Consensus(err)
    }
}

pub struct ConsensusReactor<Con>
where
    Con: Consensus,
{
    consensus: Arc<Con>,
}

impl<Con> ConsensusReactor<Con>
where
    Con: Consensus,
{
    pub fn new(consensus: Arc<Con>) -> Self {
        Self { consensus }
    }
}

impl<C> Reactor for ConsensusReactor<C>
where
    C: Consensus + 'static,
{
    type Input = (Context, ConsensusMessage);
    type Output = Reaction<ReactorMessage>;

    fn react(&mut self, _broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        if let (ctx, ConsensusMessage { message: Some(msg) }) = input {
            match msg {
                packed_message::Message::ConsensusProposal(Proposal { msg }) => {
                    let fut = self.consensus.set_vote(ctx.clone(), msg).map_err(|e| {
                        log::error!("set proposal {:?}", e);
                    });
                    Reaction::Done(Box::new(fut))
                }
                packed_message::Message::ConsensusVote(Vote { msg }) => {
                    let fut = self.consensus.set_vote(ctx.clone(), msg).map_err(|e| {
                        log::error!("set vote {:?}", e);
                    });
                    Reaction::Done(Box::new(fut))
                }
            }
        } else {
            unreachable!()
        }
    }
}
