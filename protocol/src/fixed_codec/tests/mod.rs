mod fixed_codec;

use bytes::Bytes;
use rand::random;

use crate::traits::ServiceResponse;
use crate::types::block::{Block, BlockHeader, Pill, Proof, Validator};
use crate::types::primitive::{Address, Hash, MerkleRoot};
use crate::types::receipt::{Event, Receipt, ReceiptResponse};
use crate::types::transaction::{RawTransaction, SignedTransaction, TransactionRequest};

// #####################
// Mock Primitive
// #####################

pub fn mock_hash() -> Hash {
    Hash::digest(get_random_bytes(10))
}

pub fn mock_merkle_root() -> MerkleRoot {
    Hash::digest(get_random_bytes(10))
}

pub fn mock_address() -> Address {
    let hash = mock_hash();
    Address::from_hash(hash).unwrap()
}

// #####################
// Mock Receipt
// #####################

pub fn mock_receipt_response() -> ReceiptResponse {
    ReceiptResponse {
        service_name: "mock-service".to_owned(),
        method:       "mock-method".to_owned(),
        response:     ServiceResponse::<String> {
            code:          0,
            succeed_data:  "ok".to_owned(),
            error_message: "".to_owned(),
        },
    }
}

pub fn mock_receipt() -> Receipt {
    Receipt {
        state_root:  mock_merkle_root(),
        height:      13,
        tx_hash:     mock_hash(),
        cycles_used: 100,
        events:      vec![mock_event()],
        response:    mock_receipt_response(),
    }
}

pub fn mock_event() -> Event {
    Event {
        service: "mock-event".to_owned(),
        data:    "mock-data".to_owned(),
    }
}

// #####################
// Mock Transaction
// #####################

pub fn mock_transaction_request() -> TransactionRequest {
    TransactionRequest {
        service_name: "mock-service".to_owned(),
        method:       "mock-method".to_owned(),
        payload:      "mock-payload".to_owned(),
    }
}

pub fn mock_raw_tx() -> RawTransaction {
    RawTransaction {
        chain_id:     mock_hash(),
        nonce:        mock_hash(),
        timeout:      100,
        cycles_price: 1,
        cycles_limit: 100,
        request:      mock_transaction_request(),
    }
}

pub fn mock_sign_tx() -> SignedTransaction {
    SignedTransaction {
        raw:       mock_raw_tx(),
        tx_hash:   mock_hash(),
        pubkey:    Default::default(),
        signature: Default::default(),
    }
}

// #####################
// Mock Block
// #####################

pub fn mock_validator() -> Validator {
    Validator {
        address:        mock_address(),
        propose_weight: 1,
        vote_weight:    1,
    }
}

pub fn mock_proof() -> Proof {
    Proof {
        height:     4,
        round:      99,
        block_hash: mock_hash(),
        signature:  Default::default(),
        bitmap:     Default::default(),
    }
}

pub fn mock_block_header() -> BlockHeader {
    BlockHeader {
        chain_id:                       mock_hash(),
        height:                         42,
        exec_height:                    41,
        prev_hash:                      mock_hash(),
        timestamp:                      420_000_000,
        logs_bloom:                     Default::default(),
        order_root:                     mock_merkle_root(),
        order_signed_transactions_hash: Hash::default(),
        confirm_root:                   vec![mock_hash(), mock_hash()],
        state_root:                     mock_merkle_root(),
        receipt_root:                   vec![mock_hash(), mock_hash()],
        cycles_used:                    vec![999_999],
        proposer:                       mock_address(),
        proof:                          mock_proof(),
        validator_version:              1,
        validators:                     vec![
            mock_validator(),
            mock_validator(),
            mock_validator(),
            mock_validator(),
        ],
    }
}

pub fn mock_block(order_size: usize) -> Block {
    Block {
        header:            mock_block_header(),
        ordered_tx_hashes: (0..order_size).map(|_| mock_hash()).collect(),
    }
}

pub fn mock_pill(order_size: usize, propose_size: usize) -> Pill {
    Pill {
        block:          mock_block(order_size),
        propose_hashes: (0..propose_size).map(|_| mock_hash()).collect(),
    }
}

pub fn get_random_bytes(len: usize) -> Bytes {
    let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
    Bytes::from(vec)
}
