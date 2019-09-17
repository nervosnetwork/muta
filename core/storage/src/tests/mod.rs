extern crate test;

macro_rules! exec {
    ($func: expr) => {
        futures::executor::block_on(async { $func.await.unwrap() })
    };
}

mod adapter;
mod storage;

use bytes::Bytes;
use num_traits::FromPrimitive;
use rand::random;

use protocol::types::{
    AccountAddress, Epoch, EpochHeader, Fee, Hash, Proof, RawTransaction, Receipt, ReceiptResult,
    SignedTransaction, TransactionAction,
};

fn mock_signed_tx(tx_hash: Hash) -> SignedTransaction {
    let nonce = Hash::digest(Bytes::from("XXXX"));
    let fee = Fee {
        asset_id: nonce.clone(),
        cycle:    10,
    };
    let addr_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";

    let action = TransactionAction::Transfer {
        receiver: AccountAddress::from_hex(addr_str).unwrap(),
        asset_id: nonce.clone(),
        amount:   FromPrimitive::from_i32(10).unwrap(),
    };
    let raw = RawTransaction {
        chain_id: nonce.clone(),
        nonce,
        timeout: 10,
        fee,
        action,
    };

    SignedTransaction {
        raw,
        tx_hash,
        pubkey: Default::default(),
        signature: Default::default(),
    }
}

fn mock_receipt(tx_hash: Hash) -> Receipt {
    let nonce = Hash::digest(Bytes::from("XXXX"));
    let cycles_used = Fee {
        asset_id: nonce.clone(),
        cycle:    10,
    };
    let addr_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
    let result = ReceiptResult::Transfer {
        receiver:      AccountAddress::from_hex(addr_str).unwrap(),
        before_amount: FromPrimitive::from_i32(10).unwrap(),
        after_amount:  FromPrimitive::from_i32(20).unwrap(),
    };

    Receipt {
        state_root: nonce.clone(),
        epoch_id: 10,
        tx_hash,
        cycles_used,
        result,
    }
}

fn mock_epoch(epoch_id: u64, epoch_hash: Hash) -> Epoch {
    let nonce = Hash::digest(Bytes::from("XXXX"));
    let addr_str = "10CAB8EEA4799C21379C20EF5BAA2CC8AF1BEC475B";
    let header = EpochHeader {
        chain_id: nonce.clone(),
        epoch_id,
        pre_hash: nonce.clone(),
        timestamp: 1000,
        logs_bloom: Default::default(),
        order_root: nonce.clone(),
        confirm_root: Vec::new(),
        state_root: nonce.clone(),
        receipt_root: Vec::new(),
        cycles_used: 100,
        proposer: AccountAddress::from_hex(addr_str).unwrap(),
        proof: mock_proof(epoch_hash),
        validator_version: 1,
        validators: Vec::new(),
    };

    Epoch {
        header,
        ordered_tx_hashes: Vec::new(),
    }
}

fn mock_proof(epoch_hash: Hash) -> Proof {
    Proof {
        epoch_id: 0,
        round: 0,
        epoch_hash,
        signature: Default::default(),
    }
}

fn get_random_bytes(len: usize) -> Bytes {
    let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
    Bytes::from(vec)
}
