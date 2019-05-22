// Remove this clippy bug with async await is resolved.
// ISSUE: https://github.com/rust-lang/rust-clippy/issues/3988
#![allow(clippy::needless_lifetimes)]
#![feature(async_await, try_trait)]

mod bft;
mod engine;
// mod solo;
mod synchronizer;

pub use bft::Bft;
pub use engine::Engine;
pub use synchronizer::SynchronizerManager;
// pub use solo::Solo;

use core_runtime::ConsensusError;
use core_types::{Address, Hash, Proof};

// #[derive(Debug, Deserialize)]
// pub enum ConsensusMode {
//     // Single node.
//     Solo,
//     // +2/3 byzantine consensus algorithm.
//     BFT,
// }

/// The necessary state to complete the consensus will be updated with each
/// block.
#[derive(Clone, Debug, Default)]
pub struct ConsensusStatus {
    pub height:        u64,
    pub timestamp:     u64,
    pub quota_limit:   u64,
    pub tx_limit:      u64,
    pub block_hash:    Hash,
    pub state_root:    Hash,
    pub node_address:  Address,
    pub verifier_list: Vec<Address>,
    pub proof:         Proof,
    pub interval:      u64,
}

pub type ConsensusResult<T> = Result<T, ConsensusError>;

/// The proposal from p2p, serialization and deserialization are all handled in
/// bft-rs.
pub type ProposalMessage = Vec<u8>;
// The vote from p2p, serialization and deserialization are all handled in
// bft-rs.
pub type VoteMessage = Vec<u8>;
