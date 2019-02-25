use bytes::Bytes;

use crate::{Address, Hash};

#[derive(Default, Debug, Clone)]
pub struct Transaction {
    pub to: Address,
    pub nonce: String,
    pub quota: u64,
    pub valid_until_block: u64,
    pub data: Bytes,
    pub value: Bytes,
    pub chain_id: Bytes,
    pub version: u32,
}

#[derive(Default, Debug, Clone)]
pub struct UnverifiedTransaction {
    pub transaction: Transaction,
    pub signature: Bytes,
}

#[derive(Default, Debug, Clone)]
pub struct SignedTransaction {
    pub untx: UnverifiedTransaction,
    pub hash: Hash,
    pub sender: Address,
}
