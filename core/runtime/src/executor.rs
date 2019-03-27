use std::fmt;

use cita_vm::{state::Error as StateError, Error as VMError};
use core_types::{Address, Balance, BlockHeader, Bloom, Hash, Receipt, SignedTransaction, H256};

#[derive(Default, Debug, Clone)]
pub struct ExecutionResult {
    pub state_root: Hash,
    pub all_logs_bloom: Bloom,
    pub receipts: Vec<Receipt>,
}

#[derive(Default, Debug, Clone)]
pub struct ReadonlyResult {
    pub data: Option<Vec<u8>>,
    pub error: Option<String>,
}

/// The “Executor” module determines which VM the transaction is processed by.
/// We plan to support multiple VM such as EVM, WASM, etc..
/// but their programming model must be account-based.
pub trait Executor: Send + Sync {
    /// Execute the transactions and then return the receipts, this function will modify the "state of the world".
    fn exec(
        &self,
        latest_state_root: &Hash,
        current_header: &BlockHeader,
        txs: &[SignedTransaction],
    ) -> Result<ExecutionResult, ExecutorError>;

    /// Query historical height data or perform read-only functions.
    fn readonly(
        &self,
        header: &BlockHeader,
        to: &Address,
        from: &Address,
        data: &[u8],
    ) -> Result<ReadonlyResult, ExecutorError>;

    /// Query balance of account.
    fn get_balance(&self, state_root: &Hash, address: &Address) -> Result<Balance, ExecutorError>;

    /// Query value of account.
    fn get_value(
        &self,
        state_root: &Hash,
        address: &Address,
        key: &H256,
    ) -> Result<H256, ExecutorError>;

    /// Query storage root of account.
    fn get_storage_root(&self, state_root: &Hash, address: &Address)
        -> Result<Hash, ExecutorError>;

    /// Query code of account.
    fn get_code(
        &self,
        state_root: &Hash,
        address: &Address,
    ) -> Result<(Vec<u8>, Hash), ExecutorError>;
    // fn get_proof(&self, header: &BlockHeader, address: &Address, key: &Self::Key) -> Result<Self::Value, ExecutorError>;
}

#[derive(Debug)]
pub enum ExecutorError {
    VM(VMError),
    State(StateError),
    NotFound,
    Internal(String),
}

impl fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            ExecutorError::VM(ref err) => format!("vm error: {:?}", err),
            ExecutorError::State(ref err) => format!("state error: {:?}", err),
            ExecutorError::NotFound => "not found".to_owned(),
            ExecutorError::Internal(ref err) => format!("internal error: {:?}", err),
        };
        write!(f, "{}", printable)
    }
}

impl From<VMError> for ExecutorError {
    fn from(err: VMError) -> Self {
        ExecutorError::VM(err)
    }
}

impl From<StateError> for ExecutorError {
    fn from(err: StateError) -> Self {
        ExecutorError::State(err)
    }
}
