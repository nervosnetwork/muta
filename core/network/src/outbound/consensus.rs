use core_network_message::consensus::{Proposal, Vote};
use core_network_message::Method;
use core_runtime::network::Consensus;

use crate::outbound::{BytesBroadcaster, Mode};
use crate::OutboundHandle;

impl<B, C> Consensus for OutboundHandle<B, C>
where
    B: BytesBroadcaster + Clone + Send + Sync,
    C: Send + Sync,
{
    fn proposal(&self, proposal: Vec<u8>) {
        let proposal = Proposal::from(proposal);

        self.silent_broadcast(Method::Proposal, proposal, Mode::Quick);
    }

    fn vote(&self, vote: Vec<u8>) {
        let vote = Vote::from(vote);

        self.silent_broadcast(Method::Vote, vote, Mode::Quick);
    }
}

#[cfg(test)]
mod tests {
    use core_network_message::Method;
    use core_runtime::network::Consensus;

    use crate::outbound::tests::{encode_bytes, new_outbound};
    use crate::outbound::Mode;
    use crate::p2p::Scope;

    #[test]
    fn test_proposal() {
        let data = b"sliver blade".to_vec();
        let bytes = encode_bytes(&data, Method::Proposal);

        let (outbound, _) = new_outbound::<()>();
        outbound.proposal(data);

        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, Scope::All, bytes))
        );
    }

    #[test]
    fn test_proposal_but_fail() {
        let data = b"sliver blade".to_vec();

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcaster.reply_err(true);

        outbound.proposal(data);
        assert_eq!(outbound.broadcaster.broadcasted_bytes(), None);
    }

    #[test]
    fn test_vote() {
        let data = b"vote for triss".to_vec();
        let bytes = encode_bytes(&data, Method::Vote);

        let (outbound, _) = new_outbound::<()>();
        outbound.vote(data);

        assert_eq!(
            outbound.broadcaster.broadcasted_bytes(),
            Some((Mode::Quick, Scope::All, bytes))
        );
    }

    #[test]
    fn test_vote_but_fail() {
        let data = b"vote for ciri".to_vec();

        let (outbound, _) = new_outbound::<()>();
        outbound.broadcaster.reply_err(true);

        outbound.vote(data);
        assert_eq!(outbound.broadcaster.broadcasted_bytes(), None);
    }
}
