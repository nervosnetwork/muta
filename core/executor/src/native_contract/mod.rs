mod account;
mod bank;

use lazy_static::lazy_static;

use protocol::types::Address;

lazy_static! {
    pub static ref ACCOUNT_CONTRACT_ADDRESS: Address = Address::from_hex(
        "0x23C000000000000000000000000000000000000001"
    )
    .expect("0x23C000000000000000000000000000000000000002 is not a legal native contract address.");
    pub static ref BANK_CONTRACT_ADDRESS: Address = Address::from_hex(
        "0x230000000000000000000000000000000000000002"
    )
    .expect("0x230000000000000000000000000000000000000001 is not a legal native contract address.");
}

pub use account::{NativeAccountContract, NativeAccountContractError};
pub use bank::{NativeBankContract, NativeBankContractError};
