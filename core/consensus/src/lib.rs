pub mod errors;
pub mod solo;

pub use solo::Solo;

pub enum ConsensusMode {
    // Single node.
    Solo,
    // +2/3 byzantine consensus algorithm.
    BFT,
}
