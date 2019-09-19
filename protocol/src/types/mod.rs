pub(crate) mod epoch;
pub(crate) mod primitive;
pub(crate) mod receipt;
pub(crate) mod transaction;

use std::error::Error;

use derive_more::{Display, From};

use crate::{ProtocolError, ProtocolErrorKind};

pub use epoch::{Epoch, EpochHeader, EpochId, Pill, Proof, Validator};
pub use ethbloom::{Bloom, BloomRef, Input as BloomInput};
pub use primitive::{
    Account, Address, ApprovedInfo, Asset, AssetID, AssetInfo, Balance, ContractAccount,
    ContractAddress, ContractType, Fee, Hash, MerkleRoot, UserAccount, UserAddress,
};
pub use receipt::{Receipt, ReceiptResult};
pub use transaction::{RawTransaction, SignedTransaction, TransactionAction};

#[derive(Debug, Display, From)]
pub enum TypesError {
    #[display(fmt = "Expect {:?}, get {:?}.", expect, real)]
    LengthMismatch { expect: usize, real: usize },

    #[display(fmt = "{:?}", error)]
    FromHex { error: hex::FromHexError },

    #[display(fmt = "{:?} is an invalid address", address)]
    InvalidAddress { address: String },
}

impl Error for TypesError {}

impl From<TypesError> for ProtocolError {
    fn from(error: TypesError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Types, Box::new(error))
    }
}
