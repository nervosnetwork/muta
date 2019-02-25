use bytes::Bytes;

use crate::{Address, Hash};

#[derive(Default, Debug, Clone)]
pub struct Receipt {
    pub state_root: Hash,
    pub transaction_hash: Hash,
    pub quota_used: u64,
    pub log_bloom: Bytes,
    pub logs: Vec<LogEntry>,
    pub receipt_error: String,
    pub account_nonce: u64,
}

#[derive(Default, Debug, Clone)]
pub struct LogEntry {
    pub address: Address,
    pub topics: Vec<Hash>,
    pub data: Bytes,
}
