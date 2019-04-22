// Remove this clippy bug with async await is resolved.
// ISSUE: https://github.com/rust-lang/rust-clippy/issues/3988
#![allow(clippy::needless_lifetimes)]
#![feature(async_await, await_macro, futures_api, try_trait)]

mod engine;
mod errors;
mod solo;

pub use engine::Engine;
pub use errors::ConsensusError;
pub use solo::Solo;

use old_futures::future::Future as OldFuture;

use core_context::Context;
use core_types::{Address, Hash};

#[derive(Debug)]
pub enum ConsensusMode {
    // Single node.
    Solo,
    // +2/3 byzantine consensus algorithm.
    BFT,
}

#[derive(Clone, Debug)]
pub struct ConsensusStatus {
    pub height: u64,
    pub timestamp: u64,
    pub quota_limit: u64,
    pub tx_limit: u64,
    pub block_hash: Hash,
    pub state_root: Hash,
    pub verifier_list: Vec<Address>,
}

pub type ConsensusResult<T> = Result<T, ConsensusError>;

pub type FutConsensusResult<T> = Box<OldFuture<Item = T, Error = ConsensusError> + Send>;

pub type PorposalMessage = Vec<u8>;
pub type VoteMessage = Vec<u8>;

pub trait Consensus: Send + Sync {
    fn set_proposal(&self, ctx: Context, msg: PorposalMessage) -> FutConsensusResult<()>;

    fn set_vote(&self, ctx: Context, msg: VoteMessage) -> FutConsensusResult<()>;
}
