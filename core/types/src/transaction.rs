use rlp::{Encodable, RlpStream};

use crate::{Address, Hash};

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Transaction {
    pub to:                Option<Address>,
    pub nonce:             String,
    pub quota:             u64,
    pub valid_until_block: u64,
    pub data:              Vec<u8>,
    pub value:             Vec<u8>,
    pub chain_id:          Vec<u8>,
}

/// Structure encodable to RLP
impl Encodable for Transaction {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(7);
        s.append(&self.to);
        s.append(&self.nonce);
        s.append(&self.quota);
        s.append(&self.valid_until_block);
        s.append(&self.data);
        s.append(&self.value);
        s.append(&self.chain_id);
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct UnverifiedTransaction {
    pub transaction: Transaction,
    pub signature:   Vec<u8>,
}

impl Encodable for UnverifiedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2);
        s.append(&self.transaction);
        s.append(&self.signature);
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct SignedTransaction {
    pub untx:   UnverifiedTransaction,
    pub hash:   Hash,
    pub sender: Address,
}

impl Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3);
        s.append(&self.untx);
        s.append(&self.hash);
        s.append(&self.sender);
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct TransactionPosition {
    pub block_hash: Hash,
    pub position:   u32,
}
