use futures::future::ok;

use core_context::Context;

use crate::p2p::{Broadcaster, Message as P2PMessage};
use crate::reactor::{FutReactorResult, Reactor};
use crate::Message;

#[derive(Clone, Debug)]
pub enum ConsensusMessage {
    Proposal { msg: Vec<u8> },
    Vote { msg: Vec<u8> },
}

pub struct ConsensusReactor {}

impl ConsensusReactor {
    pub fn new() -> Self {
        Self {}
    }
}

impl Reactor for ConsensusReactor {
    type Input = ConsensusMessage;
    type Output = FutReactorResult;

    fn react(&mut self, mut broadcaster: Broadcaster, input: Self::Input) -> Self::Output {
        match input {
            ConsensusMessage::Proposal { msg } => {
                broadcaster.send(
                    Context::new(),
                    P2PMessage::from(Message::BroadcastPrposal { msg }),
                );
            }
            ConsensusMessage::Vote { msg } => {
                broadcaster.send(
                    Context::new(),
                    P2PMessage::from(Message::BroadcastVote { msg }),
                );
            }
        }

        Box::new(ok(()))
    }
}

pub mod impl_comp {
    use core_consensus::{Broadcaster, ProposalMessage, VoteMessage};

    use crate::reactor::outbound::{OutboundMessage, Sender};

    use super::ConsensusMessage;

    impl Broadcaster for Sender {
        fn proposal(&mut self, proposal: ProposalMessage) {
            let msg = OutboundMessage::Consensus(ConsensusMessage::Proposal { msg: proposal });
            if let Err(err) = self.try_send(msg) {
                log::error!("consensus: broadcast proposal failure {}", err);
            }
        }

        fn vote(&mut self, vote: VoteMessage) {
            let msg = OutboundMessage::Consensus(ConsensusMessage::Vote { msg: vote });
            if let Err(err) = self.try_send(msg) {
                log::error!("consensus: broadcast vote failure {}", err);
            }
        }
    }
}
