use std::error::Error;

use derive_more::{Display, From};

use protocol::types::Hash;
use protocol::{ProtocolError, ProtocolErrorKind};

#[derive(Debug, Display, From)]
pub enum MemPoolError {
    #[display(fmt = "Tx: {:?} insert failed", tx_hash)]
    Insert { tx_hash: Hash },
    #[display(fmt = "Mempool reach limit: {}", pool_size)]
    ReachLimit { pool_size: usize },
    #[display(fmt = "Tx: {:?} exists in pool", tx_hash)]
    Dup { tx_hash: Hash },
    #[display(fmt = "Pull {} tx_hashes, return {} signed_txs", require, response)]
    EnsureBreak { require: usize, response: usize },
    #[display(
        fmt = "Return mismatch number of full transaction, require: {}, response: {}. This should not happen!",
        require,
        response
    )]
    MisMatch { require: usize, response: usize },
    #[display(
        fmt = "Transaction insert into candidate queue with len: {} failed which should not happen!",
        len
    )]
    InsertCandidate { len: usize },
}

impl Error for MemPoolError {}

impl From<MemPoolError> for ProtocolError {
    fn from(error: MemPoolError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Mempool, Box::new(error))
    }
}
