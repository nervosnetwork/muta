extern crate test;

mod codec;

use bytes::Bytes;
use num_traits::FromPrimitive;
use rand::random;

use crate::types::{
    epoch::{Epoch, EpochHeader, EpochId, Pill, Proof, Validator},
    primitive::{
        Asset, AssetID, Balance, ContractAddress, ContractType, Fee, Hash, MerkleRoot, UserAddress,
    },
    receipt::{Receipt, ReceiptResult},
    transaction::{RawTransaction, SignedTransaction, TransactionAction},
};

enum ReceiptType {
    Transfer,
    Approve,
    Deploy,
    Call,
    Fail,
}

enum AType {
    Transfer,
    Approve,
    Deploy,
    Call,
}

// #####################
// Mock Primitive
// #####################

fn mock_balance() -> Balance {
    FromPrimitive::from_i32(100).unwrap()
}

fn mock_hash() -> Hash {
    Hash::digest(get_random_bytes(10))
}

fn mock_merkle_root() -> MerkleRoot {
    Hash::digest(get_random_bytes(10))
}

fn mock_asset_id() -> AssetID {
    Hash::digest(Bytes::from("asset_id"))
}

fn mock_account_address() -> UserAddress {
    UserAddress::from_hex("10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B").unwrap()
}

fn mock_contract_address() -> ContractAddress {
    ContractAddress::from_hex("20CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B").unwrap()
}

fn mock_asset() -> Asset {
    Asset {
        id:              mock_asset_id(),
        name:            "test".to_string(),
        symbol:          "MT".to_string(),
        supply:          mock_balance(),
        manage_contract: mock_contract_address(),
        storage_root:    mock_merkle_root(),
    }
}

fn mock_fee() -> Fee {
    Fee {
        asset_id: mock_asset_id(),
        cycle:    10,
    }
}

// #####################
// Mock Receipt
// #####################

fn mock_result(rtype: ReceiptType) -> ReceiptResult {
    match rtype {
        ReceiptType::Transfer => ReceiptResult::Transfer {
            receiver:      mock_account_address(),
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

fn mock_receipt(rtype: ReceiptType) -> Receipt {
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

fn mock_action(atype: AType) -> TransactionAction {
    match atype {
        AType::Transfer => TransactionAction::Transfer {
            receiver: mock_account_address(),
            asset_id: mock_asset_id(),
            amount:   mock_balance(),
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
            contract: mock_contract_address(),
            method:   "get".to_string(),
            args:     vec![get_random_bytes(10), get_random_bytes(10)],
            asset_id: mock_asset_id(),
            amount:   mock_balance(),
        },
    }
}

fn mock_raw_tx(atype: AType) -> RawTransaction {
    RawTransaction {
        chain_id: mock_hash(),
        nonce:    mock_hash(),
        timeout:  100,
        fee:      mock_fee(),
        action:   mock_action(atype),
    }
}

fn mock_sign_tx(atype: AType) -> SignedTransaction {
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

fn mock_validator() -> Validator {
    Validator {
        address:        mock_account_address(),
        propose_weight: 1u8,
        vote_weight:    1u8,
    }
}

fn mock_proof() -> Proof {
    Proof {
        epoch_id:   0,
        round:      0,
        epoch_hash: mock_hash(),
        signature:  Default::default(),
        bitmap:     Default::default(),
    }
}

fn mock_epoch_id() -> EpochId {
    EpochId { id: 10 }
}

fn mock_epoch_header() -> EpochHeader {
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
        cycles_used:       Vec::new(),
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

fn mock_epoch(order_size: usize) -> Epoch {
    Epoch {
        header:            mock_epoch_header(),
        ordered_tx_hashes: (0..order_size).map(|_| mock_hash()).collect(),
    }
}

fn mock_pill(order_size: usize, propose_size: usize) -> Pill {
    Pill {
        epoch:          mock_epoch(order_size),
        propose_hashes: (0..propose_size).map(|_| mock_hash()).collect(),
    }
}

fn get_random_bytes(len: usize) -> Bytes {
    let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
    Bytes::from(vec)
}
