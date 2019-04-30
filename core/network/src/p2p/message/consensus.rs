use prost::Message as ProstMessage;

#[derive(Clone, PartialEq, ProstMessage)]
pub struct Proposal {
    #[prost(bytes, tag = "1")]
    pub msg: Vec<u8>,
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct Vote {
    #[prost(bytes, tag = "1")]
    pub msg: Vec<u8>,
}

pub mod packed_message {
    use prost::Oneof;

    use super::{Proposal, Vote};

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        ConsensusProposal(Proposal),

        #[prost(message, tag = "2")]
        ConsensusVote(Vote),
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct ConsensusMessage {
    #[prost(oneof = "packed_message::Message", tags = "1, 2")]
    pub message: Option<packed_message::Message>,
}

impl ConsensusMessage {
    pub fn proposal(proposal: Vec<u8>) -> Self {
        let proposal = Proposal { msg: proposal };
        ConsensusMessage {
            message: Some(packed_message::Message::ConsensusProposal(proposal)),
        }
    }

    pub fn vote(vote: Vec<u8>) -> Self {
        let vote = Vote { msg: vote };
        ConsensusMessage {
            message: Some(packed_message::Message::ConsensusVote(vote)),
        }
    }
}
