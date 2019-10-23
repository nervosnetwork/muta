mod fixed_codec;

use std::collections::BTreeMap;

use bytes::Bytes;
use num_traits::FromPrimitive;
use rand::random;

use crate::types::epoch::{Epoch, EpochHeader, EpochId, Pill, Proof, Validator};
use crate::types::genesis::{Genesis, GenesisStateAlloc, GenesisStateAsset, GenesisSystemToken};
use crate::types::primitive::{
    Account, Asset, AssetID, AssetInfo, Balance, ContractAccount, ContractAddress, ContractType,
    Fee, Hash, MerkleRoot, UserAccount, UserAddress,
};
use crate::types::receipt::{Receipt, ReceiptResult};
use crate::types::transaction::{
    CarryingAsset, RawTransaction, SignedTransaction, TransactionAction,
};

pub enum ReceiptType {
    Transfer,
    Approve,
    Deploy,
    Call,
    Fail,
}

pub enum AType {
    Transfer,
    Approve,
    Deploy,
    Call,
}

// #####################
// Mock Primitive
// #####################

pub fn mock_balance() -> Balance {
    FromPrimitive::from_i32(100).unwrap()
}

pub fn mock_hash() -> Hash {
    Hash::digest(get_random_bytes(10))
}

pub fn mock_merkle_root() -> MerkleRoot {
    Hash::digest(get_random_bytes(10))
}

pub fn mock_asset_id() -> AssetID {
    Hash::digest(Bytes::from("asset_id"))
}

pub fn mock_account_address() -> UserAddress {
    UserAddress::from_hex("10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B").unwrap()
}

pub fn mock_contract_address() -> ContractAddress {
    ContractAddress::from_hex("20CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B").unwrap()
}

pub fn mock_asset() -> Asset {
    Asset {
        id:              mock_asset_id(),
        name:            "test".to_string(),
        symbol:          "MT".to_string(),
        supply:          mock_balance(),
        manage_contract: mock_contract_address(),
        storage_root:    mock_merkle_root(),
    }
}

pub fn mock_fee() -> Fee {
    Fee {
        asset_id: mock_asset_id(),
        cycle:    10,
    }
}

pub fn mock_account_user() -> Account {
    Account::User(mock_user_account())
}

pub fn mock_account_contract() -> Account {
    Account::Contract(mock_contract_account())
}

pub fn mock_user_account() -> UserAccount {
    UserAccount {
        nonce:  8,
        assets: BTreeMap::default(),
    }
}

pub fn mock_contract_account() -> ContractAccount {
    ContractAccount {
        nonce:        8,
        assets:       BTreeMap::default(),
        storage_root: mock_hash(),
    }
}

pub fn mock_asset_info() -> AssetInfo {
    AssetInfo {
        balance:  mock_balance(),
        approved: BTreeMap::default(),
    }
}

// #####################
// Mock Receipt
// #####################

pub fn mock_result(rtype: ReceiptType) -> ReceiptResult {
    match rtype {
        ReceiptType::Transfer => ReceiptResult::Transfer {
            receiver:      mock_account_address(),
            asset_id:      mock_asset_id(),
            before_amount: mock_balance(),
            after_amount:  mock_balance(),
        },
        ReceiptType::Approve => ReceiptResult::Approve {
            spender:  mock_contract_address(),
            asset_id: mock_asset_id(),
            max:      mock_balance(),
        },
        ReceiptType::Deploy => ReceiptResult::Deploy {
            contract:      mock_contract_address(),
            contract_type: ContractType::Asset,
        },
        ReceiptType::Call => ReceiptResult::Call {
            contract:     mock_contract_address(),
            return_value: get_random_bytes(100),
            logs_bloom:   Box::new(Default::default()),
        },
        ReceiptType::Fail => ReceiptResult::Fail {
            system: "system".to_string(),
            user:   "user".to_string(),
        },
    }
}

pub fn mock_receipt(rtype: ReceiptType) -> Receipt {
    Receipt {
        state_root:  mock_merkle_root(),
        epoch_id:    13,
        tx_hash:     mock_hash(),
        cycles_used: mock_fee(),
        result:      mock_result(rtype),
    }
}

// #####################
// Mock Transaction
// #####################

pub fn mock_action(atype: AType) -> TransactionAction {
    match atype {
        AType::Transfer => TransactionAction::Transfer {
            receiver:       mock_account_address(),
            carrying_asset: CarryingAsset {
                asset_id: mock_asset_id(),
                amount:   mock_balance(),
            },
        },
        AType::Approve => TransactionAction::Approve {
            spender:  mock_contract_address(),
            asset_id: mock_asset_id(),
            max:      mock_balance(),
        },
        AType::Deploy => TransactionAction::Deploy {
            code:          get_random_bytes(100),
            contract_type: ContractType::Library,
        },
        AType::Call => TransactionAction::Call {
            contract:       mock_contract_address(),
            method:         "get".to_string(),
            args:           vec![get_random_bytes(10), get_random_bytes(10)],
            carrying_asset: Some(CarryingAsset {
                asset_id: mock_asset_id(),
                amount:   mock_balance(),
            }),
        },
    }
}

pub fn mock_raw_tx(atype: AType) -> RawTransaction {
    RawTransaction {
        chain_id: mock_hash(),
        nonce:    mock_hash(),
        timeout:  100,
        fee:      mock_fee(),
        action:   mock_action(atype),
    }
}

pub fn mock_sign_tx(atype: AType) -> SignedTransaction {
    SignedTransaction {
        raw:       mock_raw_tx(atype),
        tx_hash:   mock_hash(),
        pubkey:    Default::default(),
        signature: Default::default(),
    }
}

// #####################
// Mock Epoch
// #####################

pub fn mock_validator() -> Validator {
    Validator {
        address:        mock_account_address(),
        propose_weight: 1u8,
        vote_weight:    1u8,
    }
}

pub fn mock_proof() -> Proof {
    Proof {
        epoch_id:   4,
        round:      99,
        epoch_hash: mock_hash(),
        signature:  Default::default(),
        bitmap:     Default::default(),
    }
}

pub fn mock_epoch_id() -> EpochId {
    EpochId { id: 10 }
}

pub fn mock_epoch_header() -> EpochHeader {
    EpochHeader {
        chain_id:          mock_hash(),
        epoch_id:          42,
        pre_hash:          mock_hash(),
        timestamp:         420_000_000,
        logs_bloom:        Default::default(),
        order_root:        mock_merkle_root(),
        confirm_root:      vec![mock_hash(), mock_hash()],
        state_root:        mock_merkle_root(),
        receipt_root:      vec![mock_hash(), mock_hash()],
        cycles_used:       999_999,
        proposer:          mock_account_address(),
        proof:             mock_proof(),
        validator_version: 1,
        validators:        vec![
            mock_validator(),
            mock_validator(),
            mock_validator(),
            mock_validator(),
        ],
    }
}

pub fn mock_epoch(order_size: usize) -> Epoch {
    Epoch {
        header:            mock_epoch_header(),
        ordered_tx_hashes: (0..order_size).map(|_| mock_hash()).collect(),
    }
}

pub fn mock_pill(order_size: usize, propose_size: usize) -> Pill {
    Pill {
        epoch:          mock_epoch(order_size),
        propose_hashes: (0..propose_size).map(|_| mock_hash()).collect(),
    }
}

// #####################
// Mock Genesis
// #####################

pub fn mock_genesis() -> Genesis {
    Genesis {
        timestamp:    99,
        prevhash:     "prevhashtest".to_string(),
        system_token: GenesisSystemToken {
            code:   "codetest".to_string(),
            name:   "nametest".to_string(),
            symbol: "symbol".to_string(),
            supply: 7,
        },
        state_alloc:  vec![
            GenesisStateAlloc {
                address: "test".to_string(),
                assets:  vec![
                    GenesisStateAsset {
                        asset_id: "test".to_string(),
                        balance:  "test".to_string(),
                    },
                    GenesisStateAsset {
                        asset_id: "test".to_string(),
                        balance:  "test".to_string(),
                    },
                ],
            },
            GenesisStateAlloc {
                address: "test".to_string(),
                assets:  vec![
                    GenesisStateAsset {
                        asset_id: "test".to_string(),
                        balance:  "test".to_string(),
                    },
                    GenesisStateAsset {
                        asset_id: "test".to_string(),
                        balance:  "test".to_string(),
                    },
                ],
            },
        ],
    }
}

pub fn get_random_bytes(len: usize) -> Bytes {
    let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
    Bytes::from(vec)
}
