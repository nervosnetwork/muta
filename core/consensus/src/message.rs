use overlord::types::{AggregatedVote, SignedProposal, SignedVote};
use overlord::Codec;
use rlp::encode;
use serde::{Deserialize, Serialize};

pub const END_GOSSIP_SIGNED_PROPOSAL: &str = "/gossip/consensus/signed_proposal";
pub const END_GOSSIP_SIGNED_VOTE: &str = "/gossip/consensus/signed_vote";
pub const END_GOSSIP_AGGREGATED_VOTE: &str = "/gossip/consensus/qc";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proposal(pub Vec<u8>);

impl<C: Codec> From<SignedProposal<C>> for Proposal {
    fn from(proposal: SignedProposal<C>) -> Self {
        Proposal(encode(&proposal))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Vote(pub Vec<u8>);

impl From<SignedVote> for Vote {
    fn from(vote: SignedVote) -> Self {
        Vote(encode(&vote))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QC(pub Vec<u8>);

impl From<AggregatedVote> for QC {
    fn from(aggregated_vote: AggregatedVote) -> Self {
        QC(encode(&aggregated_vote))
    }
}
