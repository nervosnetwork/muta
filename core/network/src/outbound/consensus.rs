use core_network_message::consensus::{Proposal, Vote};
use core_network_message::Method;
use core_runtime::network::Consensus;

use crate::outbound::Mode;
use crate::{BytesBroadcaster, OutboundHandle};

impl Consensus for OutboundHandle {
    fn proposal(&self, proposal: Vec<u8>) {
        let proposal = Proposal::from(proposal);

        self.silent_broadcast(Method::Proposal, proposal, Mode::Quick);
    }

    fn vote(&self, vote: Vec<u8>) {
        let vote = Vote::from(vote);

        self.silent_broadcast(Method::Vote, vote, Mode::Quick);
    }
}
