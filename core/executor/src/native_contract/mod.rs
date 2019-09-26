mod account;
mod bank;

use lazy_static::lazy_static;

use protocol::types::ContractAddress;

lazy_static! {
    pub static ref ACCOUNT_CONTRACT_ADDRESS: ContractAddress =
        ContractAddress::from_hex("0x23C000000000000000000000000000000000000001")
            .expect("invalid ACCOUNT_CONTRACT_ADDRESS");
    pub static ref BANK_CONTRACT_ADDRESS: ContractAddress =
        ContractAddress::from_hex("0x230000000000000000000000000000000000000002")
            .expect("invalid BANK_CONTRACT_ADDRESS");
    pub static ref DEX_CONTRACT_ADDRESS: ContractAddress =
        ContractAddress::from_hex("0x230000000000000000000000000000000000000003")
            .expect("invalid DEX_CONTRACT_ADDRESS");
}

pub use account::{NativeAccountContract, NativeAccountContractError};
pub use bank::{NativeBankContract, NativeBankContractError};
