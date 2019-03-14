use std::convert::{From, Into};

use core_serialization::transaction::{
    SignedTransaction as PbSignedTransaction, Transaction as PbTransaction,
    UnverifiedTransaction as PbUnverifiedTransaction,
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

impl From<PbTransaction> for Transaction {
    fn from(tx: PbTransaction) -> Self {
        Transaction {
            to: Address::from(tx.to.as_ref()),
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
            to: self.to.as_ref().to_vec(),
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
            hash: Hash::from_raw(&signed_tx.hash),
            sender: Address::from(signed_tx.sender.as_ref()),
        }
    }
}

impl Into<PbSignedTransaction> for SignedTransaction {
    fn into(self) -> PbSignedTransaction {
        PbSignedTransaction {
            untx: Some(self.untx.clone().into()),
            hash: self.hash.as_ref().to_vec(),
            sender: self.sender.as_ref().to_vec(),
        }
    }
}
