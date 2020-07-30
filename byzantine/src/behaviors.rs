use bytes::Bytes;
use derive_more::Constructor;
use serde_derive::Deserialize;

use core_consensus::message::{
    Choke, Proposal, Vote, BROADCAST_HEIGHT, END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_CHOKE,
    END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE, QC,
};
use core_mempool::{MsgNewTxs, MsgPullTxs, END_GOSSIP_NEW_TXS, RPC_PULL_TXS};
use protocol::traits::Priority;

#[derive(Constructor, Clone, Debug)]
pub struct Behavior {
    pub msg_type: MessageType,
    pub msg_num:  u64,
    pub request:  Option<Request>,
    pub send_to:  Vec<Bytes>,
    pub priority: Priority,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Request {
    NewTx(MsgNewTxs),
    PullTxs(MsgPullTxs),
    RecvProposal(Proposal),
    RecvVote(Vote),
    RecvQC(QC),
    RecvChoke(Choke),
    RecvHeight(u64),
}

impl Request {
    pub fn to_end(&self) -> &str {
        match self {
            Request::NewTx(_) => END_GOSSIP_NEW_TXS,
            Request::PullTxs(_) => RPC_PULL_TXS,
            Request::RecvProposal(_) => END_GOSSIP_SIGNED_PROPOSAL,
            Request::RecvVote(_) => END_GOSSIP_SIGNED_VOTE,
            Request::RecvQC(_) => END_GOSSIP_AGGREGATED_VOTE,
            Request::RecvChoke(_) => END_GOSSIP_SIGNED_CHOKE,
            Request::RecvHeight(_) => BROADCAST_HEIGHT,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
pub enum MessageType {
    NewTxs(NewTx),
    SendProposal(NewProposal),
    RecvProposal(PullTxs),
    SendVote(NewVote),
    RecvVote,
    SendQC(NewQC),
    RecvQC,
    SendChoke(NewChoke),
    RecvChoke,
    SendHeight,
    RecvHeight,
    PullTxs(NewTx),
}

#[derive(Clone, Debug, Deserialize)]
pub enum NewTx {
    InvalidStruct,
    InvalidHash,
    InvalidSig,
    InvalidChainID,
    InvalidCyclesPrice,
    InvalidCyclesLimit,
    InvalidNonceOfRandLen,
    InvalidNonceDup,
    InvalidRequest,
    InvalidTimeout,
    InvalidSender,
}

#[derive(Clone, Debug, Deserialize)]
pub enum PullTxs {
    Valid,
    InvalidStruct,
    InvalidHeight,
    InvalidHash,
    NotExistTxs,
}

#[derive(Clone, Debug, Deserialize)]
pub enum NewProposal {
    Valid,
    InvalidStruct,
    InvalidChainId,
    InvalidPrevHash,
    InvalidHeight,
    InvalidExecHeight,
    InvalidTimestamp,
    InvalidOrderRoot,
    InvalidSignedTxsHash,
    InvalidConfirmRoot,
    InvalidStateRoot,
    InvalidReceiptRoot,
    InvalidCyclesUsed,
    InvalidBlockProposer,
    InvalidProof,
    InvalidVersion,
    InvalidValidators,
    InvalidTxHash,
    InvalidSig,
    InvalidProposalHeight,
    InvalidRound,
    InvalidContentStruct,
    InvalidBlockHash,
    InvalidLock,
    InvalidProposalProposer,
}

#[derive(Clone, Debug, Deserialize)]
pub enum NewVote {
    InvalidStruct,
    InvalidHeight,
    InvalidRound,
    InvalidBlockHash,
    InvalidSig,
    InvalidVoter,
}

#[derive(Clone, Debug, Deserialize)]
pub enum NewQC {
    InvalidStruct,
    InvalidHeight,
    InvalidRound,
    InvalidBlockHash,
    InvalidSig,
    InvalidLeader,
}

#[derive(Clone, Debug, Deserialize)]
pub enum NewChoke {
    InvalidStruct,
    InvalidHeight,
    InvalidRound,
    InvalidFrom,
    InvalidSig,
    InvalidAddress,
}

#[derive(Clone, Debug, Deserialize)]
pub enum SyncPullBlock {
    Valid,
}
