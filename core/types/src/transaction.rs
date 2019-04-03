use std::convert::{From, Into};

use rlp::{Encodable, RlpStream};

use core_serialization::transaction::{
    SignedTransaction as PbSignedTransaction, Transaction as PbTransaction,
    TransactionPosition as PbTransactionPosition, UnverifiedTransaction as PbUnverifiedTransaction,
};

use crate::{Address, Hash};

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Transaction {
    pub to: Address,
    pub nonce: String,
    pub quota: u64,
    pub valid_until_block: u64,
    pub data: Vec<u8>,
    pub value: Vec<u8>,
    pub chain_id: Vec<u8>,
}

/// Structure encodable to RLP
impl Encodable for Transaction {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.to);
        s.append(&self.nonce);
        s.append(&self.quota);
        s.append(&self.valid_until_block);
        s.append(&self.data);
        s.append(&self.value);
        s.append(&self.chain_id);
    }
}

impl Transaction {
    /// Calculate the block hash. To maintain consistency we use RLP serialization.
    pub fn hash(&self) -> Hash {
        let rlp_data = rlp::encode(self);
        Hash::digest(&rlp_data)
    }
}

impl From<PbTransaction> for Transaction {
    fn from(tx: PbTransaction) -> Self {
        Transaction {
            to: Address::from_bytes(&tx.to).expect("never returns an error"),
            nonce: tx.nonce,
            quota: tx.quota,
            valid_until_block: tx.valid_until_block,
            data: tx.data,
            value: tx.value,
            chain_id: tx.chain_id,
        }
    }
}

impl Into<PbTransaction> for Transaction {
    fn into(self) -> PbTransaction {
        PbTransaction {
            to: self.to.as_bytes().to_vec(),
            nonce: self.nonce,
            quota: self.quota,
            valid_until_block: self.valid_until_block,
            data: self.data,
            value: self.value,
            chain_id: self.chain_id,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct UnverifiedTransaction {
    pub transaction: Transaction,
    pub signature: Vec<u8>,
}

impl From<PbUnverifiedTransaction> for UnverifiedTransaction {
    fn from(untx: PbUnverifiedTransaction) -> Self {
        let tx = match untx.transaction {
            Some(tx) => Transaction::from(tx),
            None => Transaction::default(),
        };

        UnverifiedTransaction {
            transaction: tx,
            signature: untx.signature,
        }
    }
}

impl Into<PbUnverifiedTransaction> for UnverifiedTransaction {
    fn into(self) -> PbUnverifiedTransaction {
        PbUnverifiedTransaction {
            transaction: Some(self.transaction.clone().into()),
            signature: self.signature,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct SignedTransaction {
    pub untx: UnverifiedTransaction,
    pub hash: Hash,
    pub sender: Address,
}

impl From<PbSignedTransaction> for SignedTransaction {
    fn from(signed_tx: PbSignedTransaction) -> Self {
        let untx = match signed_tx.untx {
            Some(untx) => UnverifiedTransaction::from(untx),
            None => UnverifiedTransaction::default(),
        };

        SignedTransaction {
            untx,
            hash: Hash::from_bytes(&signed_tx.hash).expect("never returns an error"),
            sender: Address::from_bytes(&signed_tx.sender).expect("never returns an error"),
        }
    }
}

impl Into<PbSignedTransaction> for SignedTransaction {
    fn into(self) -> PbSignedTransaction {
        PbSignedTransaction {
            untx: Some(self.untx.clone().into()),
            hash: self.hash.as_bytes().to_vec(),
            sender: self.sender.as_bytes().to_vec(),
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct TransactionPosition {
    pub block_hash: Hash,
    pub position: u32,
}

impl From<PbTransactionPosition> for TransactionPosition {
    fn from(transaction_position: PbTransactionPosition) -> Self {
        TransactionPosition {
            block_hash: Hash::from_bytes(&transaction_position.block_hash)
                .expect("never returns an error"),
            position: transaction_position.position,
        }
    }
}

impl Into<PbTransactionPosition> for TransactionPosition {
    fn into(self) -> PbTransactionPosition {
        PbTransactionPosition {
            block_hash: self.block_hash.as_bytes().to_vec(),
            position: self.position,
        }
    }
}
