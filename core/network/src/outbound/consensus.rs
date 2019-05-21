use futures::prelude::{FutureExt, TryFutureExt};

use core_network_message::consensus::{Proposal, Vote};
use core_network_message::Method;
use core_runtime::network::Consensus;

use crate::outbound::Mode;
use crate::{BytesBroadcaster, OutboundHandle};

impl Consensus for OutboundHandle {
    fn proposal(&self, proposal: Vec<u8>) {
        let outbound = self.clone();

        let job = async move {
            let proposal = Proposal::from(proposal);

            outbound.silent_broadcast(Method::Proposal, proposal, Mode::Quick);
        };

        tokio::spawn(job.unit_error().boxed().compat());
    }

    fn vote(&self, vote: Vec<u8>) {
        let outbound = self.clone();

        let job = async move {
            let vote = Vote::from(vote);

            outbound.silent_broadcast(Method::Vote, vote, Mode::Quick);
        };

        tokio::spawn(job.unit_error().boxed().compat());
    }
}
