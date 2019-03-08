use std::convert::{From, Into};

use core_serialization::receipt::{LogEntry as PbLogEntry, Receipt as PbReceipt};

use crate::{Address, Hash};

#[derive(Default, Debug, Clone)]
pub struct Receipt {
    pub state_root: Hash,
    pub transaction_hash: Hash,
    pub quota_used: u64,
    pub log_bloom: Vec<u8>,
    pub logs: Vec<LogEntry>,
    pub receipt_error: String,
    pub account_nonce: u64,
}

impl From<PbReceipt> for Receipt {
    fn from(receipt: PbReceipt) -> Self {
        Receipt {
            state_root: Hash::from_raw(&receipt.state_root),
            transaction_hash: Hash::from_raw(&receipt.transaction_hash),
            quota_used: receipt.quota_used,
            log_bloom: receipt.log_bloom,
            logs: receipt
                .logs
                .iter()
                .map(|entry| LogEntry::from(entry.clone()))
                .collect(),
            receipt_error: receipt.error,
            account_nonce: receipt.account_nonce,
        }
    }
}

impl Into<PbReceipt> for Receipt {
    fn into(self) -> PbReceipt {
        PbReceipt {
            state_root: self.state_root.as_ref().to_vec(),
            transaction_hash: self.transaction_hash.as_ref().to_vec(),
            quota_used: self.quota_used,
            log_bloom: self.log_bloom,
            logs: self.logs.iter().map(|l| l.clone().into()).collect(),
            error: self.receipt_error,
            account_nonce: self.account_nonce,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct LogEntry {
    pub address: Address,
    pub topics: Vec<Hash>,
    pub data: Vec<u8>,
}

impl From<PbLogEntry> for LogEntry {
    fn from(entry: PbLogEntry) -> Self {
        LogEntry {
            address: Address::from(entry.address.as_ref()),
            topics: entry.topics.iter().map(|t| Hash::from_raw(t)).collect(),
            data: entry.data,
        }
    }
}

impl Into<PbLogEntry> for LogEntry {
    fn into(self) -> PbLogEntry {
        PbLogEntry {
            address: self.address.as_ref().to_vec(),
            topics: self.topics.iter().map(|h| h.as_ref().to_vec()).collect(),
            data: self.data,
        }
    }
}
