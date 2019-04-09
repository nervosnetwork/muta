use std::error;
use std::fmt;

use bytes::{BytesMut, IntoBuf};
use core_types::{Address, Bloom, Hash};
use futures::future::{result, Future};
use prost::{DecodeError, EncodeError, Message};

#[derive(Default)]
pub struct AsyncCodec;

impl AsyncCodec {
    pub fn decode<T: 'static + Message + Default>(
        data: Vec<u8>,
    ) -> Box<Future<Item = T, Error = CodecError> + Send> {
        Box::new(result(SyncCodec::decode(data)))
    }

    pub fn encode<T: Message>(
        msg: &T,
        buf: &mut BytesMut,
    ) -> Box<Future<Item = (), Error = CodecError> + Send> {
        Box::new(result(SyncCodec::encode(msg, buf)))
    }

    pub fn encoded_len<T: Message>(msg: &T) -> usize {
        SyncCodec::encoded_len(msg)
    }
}

#[derive(Default)]
pub struct SyncCodec;

impl SyncCodec {
    pub fn decode<T: 'static + Message + Default>(data: Vec<u8>) -> Result<T, CodecError> {
        T::decode(data.into_buf()).map_err(CodecError::Decode)
    }

    pub fn encode<T: Message>(msg: &T, buf: &mut BytesMut) -> Result<(), CodecError> {
        msg.encode(buf).map_err(CodecError::Encode)?;
        Ok(())
    }

    pub fn encoded_len<T: Message>(msg: &T) -> usize {
        msg.encoded_len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Decode(DecodeError),
    Encode(EncodeError),
}

impl error::Error for CodecError {}
impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CodecError::Decode(ref err) => format!("serialization decode error: {:?}", err),
            CodecError::Encode(ref err) => format!("serialization encode error: {:?}", err),
        };
        write!(f, "{}", printable)
    }
}

impl From<DecodeError> for CodecError {
    fn from(err: DecodeError) -> Self {
        CodecError::Decode(err)
    }
}

impl From<EncodeError> for CodecError {
    fn from(err: EncodeError) -> Self {
        CodecError::Encode(err)
    }
}

// ----------------------------------------------------------------------------
// To generate .rs files, prost-build used from build.rs.
//
// Build target:
//   - /target/debug/build/core-serialization-****/block.rs
//   - /target/debug/build/core-serialization-****/receipt.rs
//   - /target/debug/build/core-serialization-****/transaction.rs
//
// Structs:
//  - [block.rs] BlockHeader
//  - [block.rs] Block
//  - [receipt.rs] LogEntry
//  - [receipt.rs] Receipt
//  - [transaction.rs] Transaction
//  - [transaction.rs] UnverifiedTransaction
//  - [transaction.rs] SignedTransaction
//  - [transaction.rs] TransactionPosition
//  - ...
// ----------------------------------------------------------------------------

macro_rules! generate_module_for {
    ([$( $name:ident, )+]) => {
        $( generate_module_for!($name); )+
    };
    ([$( $name:ident ),+]) => {
        $( generate_module_for!($name); )+
    };
    ($name:ident) => {
        include!(concat!(env!("OUT_DIR"), "/", stringify!($name), ".rs"));
    };
}

generate_module_for!([block, transaction, receipt,]);

impl From<core_types::BlockHeader> for BlockHeader {
    fn from(header: core_types::BlockHeader) -> Self {
        Self {
            prevhash: header.prevhash.as_bytes().to_vec(),
            timestamp: header.timestamp,
            height: header.height,
            transactions_root: header.transactions_root.as_bytes().to_vec(),
            state_root: header.state_root.as_bytes().to_vec(),
            receipts_root: header.receipts_root.as_bytes().to_vec(),
            logs_bloom: header.logs_bloom.as_bytes().to_vec(),
            quota_used: header.quota_used,
            quota_limit: header.quota_limit,
            votes: header
                .votes
                .into_iter()
                .map(|v| v.as_bytes().to_vec())
                .collect(),
            proposer: header.proposer.as_bytes().to_vec(),
        }
    }
}

impl Into<core_types::BlockHeader> for BlockHeader {
    fn into(self) -> core_types::BlockHeader {
        core_types::BlockHeader {
            prevhash: Hash::from_bytes(&self.prevhash).expect("never returns an error"),
            timestamp: self.timestamp,
            height: self.height,
            transactions_root: Hash::from_bytes(&self.transactions_root)
                .expect("never returns an error"),
            state_root: Hash::from_bytes(&self.state_root).expect("never returns an error"),
            receipts_root: Hash::from_bytes(&self.receipts_root).expect("never returns an error"),
            logs_bloom: Bloom::from_slice(&self.logs_bloom),
            quota_used: self.quota_used,
            quota_limit: self.quota_limit,
            votes: self
                .votes
                .into_iter()
                .map(|v| Hash::from_bytes(&v).expect("never returns an error"))
                .collect(),
            proposer: Address::from_bytes(&self.proposer).expect("never returns an error"),
        }
    }
}

impl From<core_types::Block> for Block {
    fn from(block: core_types::Block) -> Self {
        Self {
            header: Some(block.header.into()),
            tx_hashes: block
                .tx_hashes
                .into_iter()
                .map(|h| h.as_bytes().to_vec())
                .collect(),
        }
    }
}

impl Into<core_types::Block> for Block {
    fn into(self) -> core_types::Block {
        let header = match self.header {
            Some(header) => header.into(),
            None => core_types::BlockHeader::default(),
        };
        core_types::Block {
            header,
            tx_hashes: self
                .tx_hashes
                .into_iter()
                .map(|tx_hash| Hash::from_bytes(&tx_hash).expect("never returns an error"))
                .collect(),
        }
    }
}

impl From<core_types::LogEntry> for LogEntry {
    fn from(entry: core_types::LogEntry) -> Self {
        Self {
            address: entry.address.as_bytes().to_vec(),
            topics: entry
                .topics
                .into_iter()
                .map(|h| h.as_bytes().to_vec())
                .collect(),
            data: entry.data,
        }
    }
}

impl Into<core_types::LogEntry> for LogEntry {
    fn into(self) -> core_types::LogEntry {
        core_types::LogEntry {
            address: Address::from_bytes(&self.address).expect("never returns an error"),
            topics: self
                .topics
                .into_iter()
                .map(|t| Hash::from_bytes(&t).expect("never returns an error"))
                .collect(),
            data: self.data,
        }
    }
}

impl From<core_types::Receipt> for Receipt {
    fn from(receipt: core_types::Receipt) -> Receipt {
        Self {
            state_root: receipt.state_root.as_bytes().to_vec(),
            transaction_hash: receipt.transaction_hash.as_bytes().to_vec(),
            block_hash: receipt.block_hash.as_bytes().to_vec(),
            quota_used: receipt.quota_used,
            logs_bloom: receipt.logs_bloom.as_bytes().to_vec(),
            logs: receipt.logs.into_iter().map(Into::into).collect(),
            error: receipt.receipt_error,
            contract_address: match receipt.contract_address {
                Some(v) => v.as_bytes().to_vec(),
                None => vec![],
            },
        }
    }
}

impl Into<core_types::Receipt> for Receipt {
    fn into(self) -> core_types::Receipt {
        core_types::Receipt {
            state_root: Hash::from_bytes(&self.state_root).expect("never returns an error"),
            transaction_hash: Hash::from_bytes(&self.transaction_hash)
                .expect("never returns an error"),
            block_hash: Hash::from_bytes(&self.block_hash).expect("never returns an error"),
            quota_used: self.quota_used,
            logs_bloom: Bloom::from_slice(&self.logs_bloom),
            logs: self.logs.into_iter().map(LogEntry::into).collect(),
            receipt_error: self.error,
            contract_address: if self.contract_address.is_empty() {
                None
            } else {
                Some(Address::from_bytes(&self.contract_address).expect("never returns an error"))
            },
        }
    }
}

impl From<core_types::Transaction> for Transaction {
    fn from(tx: core_types::Transaction) -> Self {
        Self {
            to: tx.to.as_bytes().to_vec(),
            nonce: tx.nonce,
            quota: tx.quota,
            valid_until_block: tx.valid_until_block,
            data: tx.data,
            value: tx.value,
            chain_id: tx.chain_id,
        }
    }
}

impl Into<core_types::Transaction> for Transaction {
    fn into(self) -> core_types::Transaction {
        core_types::Transaction {
            to: Address::from_bytes(&self.to).expect("never returns an error"),
            nonce: self.nonce,
            quota: self.quota,
            valid_until_block: self.valid_until_block,
            data: self.data,
            value: self.value,
            chain_id: self.chain_id,
        }
    }
}

impl From<core_types::UnverifiedTransaction> for UnverifiedTransaction {
    fn from(untx: core_types::UnverifiedTransaction) -> Self {
        Self {
            transaction: Some(Transaction::from(untx.transaction)),
            signature: untx.signature,
        }
    }
}

impl Into<core_types::UnverifiedTransaction> for UnverifiedTransaction {
    fn into(self) -> core_types::UnverifiedTransaction {
        let tx = match self.transaction {
            Some(tx) => tx.into(),
            None => core_types::Transaction::default(),
        };
        core_types::UnverifiedTransaction {
            transaction: tx,
            signature: self.signature,
        }
    }
}

impl From<core_types::SignedTransaction> for SignedTransaction {
    fn from(signed_tx: core_types::SignedTransaction) -> Self {
        Self {
            untx: Some(UnverifiedTransaction::from(signed_tx.untx)),
            hash: signed_tx.hash.as_bytes().to_vec(),
            sender: signed_tx.sender.as_bytes().to_vec(),
        }
    }
}

impl Into<core_types::SignedTransaction> for SignedTransaction {
    fn into(self) -> core_types::SignedTransaction {
        let untx = match self.untx {
            Some(untx) => untx.into(),
            None => core_types::UnverifiedTransaction::default(),
        };
        core_types::SignedTransaction {
            untx,
            hash: Hash::from_bytes(&self.hash).expect("never returns an error"),
            sender: Address::from_bytes(&self.sender).expect("never returns an error"),
        }
    }
}

impl From<core_types::TransactionPosition> for TransactionPosition {
    fn from(tp: core_types::TransactionPosition) -> Self {
        Self {
            block_hash: tp.block_hash.as_bytes().to_vec(),
            position: tp.position,
        }
    }
}

impl Into<core_types::TransactionPosition> for TransactionPosition {
    fn into(self) -> core_types::TransactionPosition {
        core_types::TransactionPosition {
            block_hash: Hash::from_bytes(&self.block_hash).expect("never returns an error"),
            position: self.position,
        }
    }
}
