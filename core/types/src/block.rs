use rlp::{Encodable, RlpStream};

use crate::{Address, Bloom, Hash};

#[derive(Default, Debug, Clone)]
pub struct BlockHeader {
    pub prevhash:          Hash,
    pub timestamp:         u64,
    pub height:            u64,
    pub transactions_root: Hash,
    pub state_root:        Hash,
    pub receipts_root:     Hash,
    pub logs_bloom:        Bloom,
    pub quota_used:        u64,
    pub quota_limit:       u64,
    pub proof:             Proof,
    pub proposer:          Address,
}

impl BlockHeader {
    /// Calculate the block header hash. To maintain consistency we use RLP
    /// serialization.
    pub fn hash(&self) -> Hash {
        let rlp_data = rlp::encode(self);
        Hash::digest(&rlp_data)
    }
}

/// Structure encodable to RLP
impl Encodable for BlockHeader {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(11);
        s.append(&self.prevhash);
        s.append(&self.timestamp);
        s.append(&self.height);
        s.append(&self.transactions_root);
        s.append(&self.state_root);
        s.append(&self.receipts_root);
        s.append(&self.logs_bloom.as_ref());
        s.append(&self.quota_used);
        s.append(&self.quota_limit);
        s.append(&self.proof);
        s.append(&self.proposer);
    }
}

#[derive(Default, Debug, Clone)]
pub struct Block {
    pub header:    BlockHeader,
    pub tx_hashes: Vec<Hash>,
    pub hash:      Hash,
}

#[derive(Default, Debug, Clone)]
pub struct Proposal {
    pub prevhash:         Hash,
    pub timestamp:        u64,
    pub height:           u64,
    pub quota_limit:      u64,
    pub proposer:         Address,
    pub transaction_root: Hash,
    pub tx_hashes:        Vec<Hash>,
    pub proof:            Proof,
}

impl Proposal {
    pub fn hash(&self) -> Hash {
        let rlp_data = rlp::encode(self);
        Hash::digest(&rlp_data)
    }
}

/// Structure encodable to RLP
impl Encodable for Proposal {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(7);
        s.append(&self.prevhash);
        s.append(&self.timestamp);
        s.append(&self.height);
        s.append(&self.quota_limit);
        s.append(&self.proposer);
        s.append(&self.transaction_root);
        s.append(&self.proof);
    }
}

#[derive(Default, Debug, Clone)]
pub struct Proof {
    pub height:        u64,
    pub round:         u64,
    pub proposal_hash: Hash,
    pub commits:       Vec<Vote>,
}

/// Structure encodable to RLP
impl Encodable for Proof {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.height);
        s.append(&self.round);
        s.append(&self.proposal_hash);
        s.append_list(&self.commits);
    }
}

#[derive(Default, Debug, Clone)]
pub struct Vote {
    pub address:   Address,
    pub signature: Vec<u8>,
}

/// Structure encodable to RLP
impl Encodable for Vote {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2);
        s.append(&self.address);
        s.append(&self.signature);
    }
}
