use std::convert::{From, Into};

use rlp::{Encodable, RlpStream};

use core_serialization::receipt::{LogEntry as PbLogEntry, Receipt as PbReceipt};

use crate::{Address, Bloom, Hash};

#[derive(Default, Debug, Clone)]
pub struct Receipt {
    pub state_root: Hash,
    pub transaction_hash: Hash,
    pub block_hash: Hash,
    pub quota_used: u64,
    pub logs_bloom: Bloom,
    pub logs: Vec<LogEntry>,
    pub receipt_error: String,
    pub contract_address: Option<Address>,
}

impl Receipt {
    /// Calculate the receipt hash. To maintain consistency we use RLP serialization.
    pub fn hash(&self) -> Hash {
        let rlp_data = rlp::encode(self);
        Hash::digest(&rlp_data)
    }
}

/// Structure encodable to RLP
impl Encodable for Receipt {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.state_root);
        s.append(&self.transaction_hash);
        s.append(&self.block_hash);
        s.append(&self.quota_used);
        s.append(&self.logs_bloom.as_bytes());
        s.append_list(&self.logs);
        s.append(&self.receipt_error);
        s.append(&self.contract_address);
    }
}

impl From<PbReceipt> for Receipt {
    fn from(receipt: PbReceipt) -> Self {
        Receipt {
            state_root: Hash::from_bytes(&receipt.state_root).expect("never returns an error"),
            transaction_hash: Hash::from_bytes(&receipt.transaction_hash)
                .expect("never returns an error"),
            block_hash: Hash::from_bytes(&receipt.block_hash).expect("never returns an error"),
            quota_used: receipt.quota_used,
            logs_bloom: Bloom::from_slice(&receipt.logs_bloom),
            logs: receipt.logs.into_iter().map(LogEntry::from).collect(),
            receipt_error: receipt.error,
            contract_address: if receipt.contract_address.is_empty() {
                None
            } else {
                Some(
                    Address::from_bytes(&receipt.contract_address).expect("never returns an error"),
                )
            },
        }
    }
}

impl Into<PbReceipt> for Receipt {
    fn into(self) -> PbReceipt {
        PbReceipt {
            state_root: self.state_root.as_bytes().to_vec(),
            transaction_hash: self.transaction_hash.as_bytes().to_vec(),
            block_hash: self.transaction_hash.as_bytes().to_vec(),
            quota_used: self.quota_used,
            logs_bloom: self.logs_bloom.as_bytes().to_vec(),
            logs: self.logs.into_iter().map(Into::into).collect(),
            error: self.receipt_error,
            contract_address: match self.contract_address {
                Some(v) => v.as_bytes().to_vec(),
                None => vec![],
            },
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
            address: Address::from_bytes(&entry.address).expect("never returns an error"),
            topics: entry
                .topics
                .iter()
                .map(|t| Hash::from_bytes(t).expect("never returns an error"))
                .collect(),
            data: entry.data,
        }
    }
}

impl Into<PbLogEntry> for LogEntry {
    fn into(self) -> PbLogEntry {
        PbLogEntry {
            address: self.address.as_bytes().to_vec(),
            topics: self
                .topics
                .into_iter()
                .map(|h| h.as_bytes().to_vec())
                .collect(),
            data: self.data,
        }
    }
}

/// Structure encodable to RLP
impl Encodable for LogEntry {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.address);
        s.append_list(&self.topics);
        s.append(&self.data);
    }
}
