#![feature(async_await, await_macro, futures_api, try_trait)]

use std::convert::TryInto;
use std::error;
use std::fmt;
use std::future::Future;
use std::iter::FromIterator;
use std::option::NoneError;

use bytes::{BytesMut, IntoBuf};
use futures::{
    future,
    stream::{FuturesOrdered, TryStreamExt},
};
use prost::{DecodeError, EncodeError, Message};

use core_types::{Address, Bloom, Hash, TypesError};

#[derive(Default)]
pub struct AsyncCodec;

impl AsyncCodec {
    pub fn decode<T: Message + Default>(
        data: Vec<u8>,
    ) -> impl Future<Output = Result<T, CodecError>> + Send {
        future::ready(SyncCodec::decode::<T>(data))
    }

    pub fn decode_batch<T: Message + Default>(
        values: Vec<Vec<u8>>,
    ) -> impl Future<Output = Result<Vec<T>, CodecError>> + Send {
        async move {
            let iter = values.into_iter().map(AsyncCodec::decode::<T>);

            let ser_values: Result<Vec<T>, CodecError> =
                await!(FuturesOrdered::from_iter(iter).try_collect());
            ser_values
        }
    }

    pub fn encode<T: Message>(msg: T) -> impl Future<Output = Result<Vec<u8>, CodecError>> + Send {
        future::ready(SyncCodec::encode::<T>(msg))
    }

    pub fn encode_batch<T: Message>(
        msgs: Vec<T>,
    ) -> impl Future<Output = Result<Vec<Vec<u8>>, CodecError>> + Send {
        async move {
            let iter = msgs.into_iter().map(AsyncCodec::encode::<T>);

            let values: Result<Vec<Vec<u8>>, CodecError> =
                await!(FuturesOrdered::from_iter(iter).try_collect());
            values
        }
    }
}

#[derive(Default)]
pub struct SyncCodec;

impl SyncCodec {
    pub fn decode<T: Message + Default>(data: Vec<u8>) -> Result<T, CodecError> {
        T::decode(data.into_buf()).map_err(CodecError::Decode)
    }

    pub fn decode_batch<T: Message + Default>(values: Vec<Vec<u8>>) -> Result<Vec<T>, CodecError> {
        let mut ser_list = Vec::with_capacity(values.len());

        for value in values.into_iter() {
            let ser = SyncCodec::decode::<T>(value)?;
            ser_list.push(ser);
        }
        Ok(ser_list)
    }

    pub fn encode<T: Message>(msg: T) -> Result<Vec<u8>, CodecError> {
        let mut buf = BytesMut::with_capacity(msg.encoded_len());
        msg.encode(&mut buf).map_err(CodecError::Encode)?;
        Ok(buf.to_vec())
    }

    pub fn encode_batch<T: Message>(msgs: Vec<T>) -> Result<Vec<Vec<u8>>, CodecError> {
        let mut encoded_values = Vec::with_capacity(msgs.len());

        for msg in msgs {
            let encoded_value = SyncCodec::encode::<T>(msg)?;
            encoded_values.push(encoded_value);
        }
        Ok(encoded_values)
    }
}

#[derive(Debug)]
pub enum CodecError {
    Decode(DecodeError),
    Encode(EncodeError),
    Types(TypesError),
    None(NoneError),
}

impl error::Error for CodecError {}
impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            CodecError::Decode(ref err) => format!("serialization decode error: {:?}", err),
            CodecError::Encode(ref err) => format!("serialization encode error: {:?}", err),
            CodecError::Types(ref err) => format!("serialization types error: {:?}", err),
            CodecError::None(ref err) => format!("serialization none error: {:?}", err),
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

impl From<TypesError> for CodecError {
    fn from(err: TypesError) -> Self {
        CodecError::Types(err)
    }
}

impl From<NoneError> for CodecError {
    fn from(err: NoneError) -> Self {
        CodecError::None(err)
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
//  - [block.rs] Proposal
//  - [block.rs] Proof
//  - [block.rs] Vote
//  - [receipt.rs] LogEntry
//  - [receipt.rs] Receipt
//  - [transaction.rs] Transaction
//  - [transaction.rs] UnverifiedTransaction
//  - [transaction.rs] SignedTransaction
//  - [transaction.rs] TransactionPosition
//  - ...
// ----------------------------------------------------------------------------

#[macro_export]
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
            prevhash:          header.prevhash.as_bytes().to_vec(),
            timestamp:         header.timestamp,
            height:            header.height,
            transactions_root: header.transactions_root.as_bytes().to_vec(),
            state_root:        header.state_root.as_bytes().to_vec(),
            receipts_root:     header.receipts_root.as_bytes().to_vec(),
            logs_bloom:        header.logs_bloom.as_bytes().to_vec(),
            quota_used:        header.quota_used,
            quota_limit:       header.quota_limit,
            proof:             Some(header.proof.into()),
            proposer:          header.proposer.as_bytes().to_vec(),
        }
    }
}

impl TryInto<core_types::BlockHeader> for BlockHeader {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::BlockHeader, Self::Error> {
        Ok(core_types::BlockHeader {
            prevhash:          Hash::from_bytes(&self.prevhash)?,
            timestamp:         self.timestamp,
            height:            self.height,
            transactions_root: Hash::from_bytes(&self.transactions_root)?,
            state_root:        Hash::from_bytes(&self.state_root)?,
            receipts_root:     Hash::from_bytes(&self.receipts_root)?,
            logs_bloom:        Bloom::from_slice(&self.logs_bloom),
            quota_used:        self.quota_used,
            quota_limit:       self.quota_limit,
            proof:             self.proof?.try_into()?,
            proposer:          Address::from_bytes(&self.proposer)?,
        })
    }
}

impl From<core_types::Block> for Block {
    fn from(block: core_types::Block) -> Self {
        Self {
            header:    Some(block.header.into()),
            tx_hashes: block
                .tx_hashes
                .into_iter()
                .map(|h| h.as_bytes().to_vec())
                .collect(),
            hash:      block.hash.as_bytes().to_vec(),
        }
    }
}

impl TryInto<core_types::Block> for Block {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::Block, Self::Error> {
        let header = self.header.ok_or(NoneError)?.try_into()?;

        let tx_hashes = self
            .tx_hashes
            .into_iter()
            .map(|tx_hash| Hash::from_bytes(&tx_hash))
            .collect::<Result<Vec<Hash>, TypesError>>()?;
        let hash = Hash::from_bytes(&self.hash)?;
        Ok(core_types::Block {
            header,
            tx_hashes,
            hash,
        })
    }
}

impl From<core_types::Proposal> for Proposal {
    fn from(proposal: core_types::Proposal) -> Self {
        Self {
            prevhash:    proposal.prevhash.as_bytes().to_vec(),
            timestamp:   proposal.timestamp,
            height:      proposal.height,
            quota_limit: proposal.quota_limit,
            proposer:    proposal.proposer.as_bytes().to_vec(),
            tx_hashes:   proposal
                .tx_hashes
                .into_iter()
                .map(|h| h.as_bytes().to_vec())
                .collect(),
            proof:       Some(proposal.proof.into()),
        }
    }
}

impl TryInto<core_types::Proposal> for Proposal {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::Proposal, Self::Error> {
        let tx_hashes = self
            .tx_hashes
            .into_iter()
            .map(|h| Hash::from_bytes(&h))
            .collect::<Result<Vec<Hash>, TypesError>>()?;
        let proof = self.proof.ok_or(NoneError)?.try_into()?;

        Ok(core_types::Proposal {
            prevhash: Hash::from_bytes(&self.prevhash)?,
            timestamp: self.timestamp,
            height: self.height,
            quota_limit: self.quota_limit,
            proposer: Address::from_bytes(&self.proposer)?,
            tx_hashes,
            proof,
        })
    }
}

impl From<core_types::Proof> for Proof {
    fn from(proof: core_types::Proof) -> Self {
        let commits: Vec<Vote> = proof.commits.into_iter().map(Into::into).collect();

        Self {
            height: proof.height,
            round: proof.round,
            proposal_hash: proof.proposal_hash.as_bytes().to_vec(),
            commits,
        }
    }
}

impl TryInto<core_types::Proof> for Proof {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::Proof, Self::Error> {
        let commits = self
            .commits
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<core_types::Vote>, CodecError>>()?;

        Ok(core_types::Proof {
            height: self.height,
            round: self.round,
            proposal_hash: Hash::from_bytes(&self.proposal_hash)?,
            commits,
        })
    }
}

impl From<core_types::Vote> for Vote {
    fn from(vote: core_types::Vote) -> Self {
        Self {
            address:   vote.address.as_bytes().to_vec(),
            signature: vote.signature,
        }
    }
}

impl TryInto<core_types::Vote> for Vote {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::Vote, Self::Error> {
        Ok(core_types::Vote {
            address:   Address::from_bytes(&self.address)?,
            signature: self.signature,
        })
    }
}

impl From<core_types::LogEntry> for LogEntry {
    fn from(entry: core_types::LogEntry) -> Self {
        Self {
            address: entry.address.as_bytes().to_vec(),
            topics:  entry
                .topics
                .into_iter()
                .map(|h| h.as_bytes().to_vec())
                .collect(),
            data:    entry.data,
        }
    }
}

impl TryInto<core_types::LogEntry> for LogEntry {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::LogEntry, Self::Error> {
        let topics = self
            .topics
            .into_iter()
            .map(|t| Hash::from_bytes(&t))
            .collect::<Result<Vec<Hash>, TypesError>>()?;

        Ok(core_types::LogEntry {
            address: Address::from_bytes(&self.address)?,
            data: self.data,
            topics,
        })
    }
}

impl From<core_types::Receipt> for Receipt {
    fn from(receipt: core_types::Receipt) -> Receipt {
        Self {
            state_root:       receipt.state_root.as_bytes().to_vec(),
            transaction_hash: receipt.transaction_hash.as_bytes().to_vec(),
            quota_used:       receipt.quota_used,
            logs_bloom:       receipt.logs_bloom.as_bytes().to_vec(),
            logs:             receipt.logs.into_iter().map(Into::into).collect(),
            error:            receipt.receipt_error,
            contract_address: match receipt.contract_address {
                Some(v) => v.as_bytes().to_vec(),
                None => vec![],
            },
        }
    }
}

impl TryInto<core_types::Receipt> for Receipt {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::Receipt, Self::Error> {
        let logs = self
            .logs
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<core_types::LogEntry>, CodecError>>()?;

        Ok(core_types::Receipt {
            state_root: Hash::from_bytes(&self.state_root)?,
            transaction_hash: Hash::from_bytes(&self.transaction_hash)?,
            quota_used: self.quota_used,
            logs_bloom: Bloom::from_slice(&self.logs_bloom),
            receipt_error: self.error,
            contract_address: if self.contract_address.is_empty() {
                None
            } else {
                Some(Address::from_bytes(&self.contract_address)?)
            },
            logs,
        })
    }
}

impl From<core_types::Transaction> for Transaction {
    fn from(tx: core_types::Transaction) -> Self {
        let to = match tx.to {
            Some(data) => data.as_bytes().to_vec(),
            None => vec![],
        };
        Self {
            to,
            nonce: tx.nonce,
            quota: tx.quota,
            valid_until_block: tx.valid_until_block,
            data: tx.data,
            value: tx.value,
            chain_id: tx.chain_id,
        }
    }
}

impl TryInto<core_types::Transaction> for Transaction {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::Transaction, Self::Error> {
        let to = if self.to.is_empty() {
            None
        } else {
            Some(Address::from_bytes(&self.to)?)
        };
        Ok(core_types::Transaction {
            to,
            nonce: self.nonce,
            quota: self.quota,
            valid_until_block: self.valid_until_block,
            data: self.data,
            value: self.value,
            chain_id: self.chain_id,
        })
    }
}

impl From<core_types::UnverifiedTransaction> for UnverifiedTransaction {
    fn from(untx: core_types::UnverifiedTransaction) -> Self {
        Self {
            transaction: Some(Transaction::from(untx.transaction)),
            signature:   untx.signature,
        }
    }
}

impl TryInto<core_types::UnverifiedTransaction> for UnverifiedTransaction {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::UnverifiedTransaction, Self::Error> {
        let tx = self.transaction.ok_or(NoneError)?.try_into()?;

        Ok(core_types::UnverifiedTransaction {
            transaction: tx,
            signature:   self.signature,
        })
    }
}

impl From<core_types::SignedTransaction> for SignedTransaction {
    fn from(signed_tx: core_types::SignedTransaction) -> Self {
        Self {
            untx:   Some(UnverifiedTransaction::from(signed_tx.untx)),
            hash:   signed_tx.hash.as_bytes().to_vec(),
            sender: signed_tx.sender.as_bytes().to_vec(),
        }
    }
}

impl TryInto<core_types::SignedTransaction> for SignedTransaction {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::SignedTransaction, Self::Error> {
        let untx = self.untx.ok_or(NoneError)?.try_into()?;

        Ok(core_types::SignedTransaction {
            untx,
            hash: Hash::from_bytes(&self.hash)?,
            sender: Address::from_bytes(&self.sender)?,
        })
    }
}

impl From<core_types::TransactionPosition> for TransactionPosition {
    fn from(tp: core_types::TransactionPosition) -> Self {
        Self {
            block_hash: tp.block_hash.as_bytes().to_vec(),
            position:   tp.position,
        }
    }
}

impl TryInto<core_types::TransactionPosition> for TransactionPosition {
    type Error = CodecError;

    fn try_into(self) -> Result<core_types::TransactionPosition, Self::Error> {
        Ok(core_types::TransactionPosition {
            block_hash: Hash::from_bytes(&self.block_hash)?,
            position:   self.position,
        })
    }
}
