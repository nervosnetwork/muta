use rlp::{Encodable, RlpStream};

use crate::{Address, Bloom, Hash};

#[derive(Default, Debug, Clone)]
pub struct BlockHeader {
    pub prevhash: Hash,
    pub timestamp: u64,
    pub height: u64,
    pub transactions_root: Hash,
    pub state_root: Hash,
    pub receipts_root: Hash,
    pub logs_bloom: Bloom,
    pub quota_used: u64,
    pub quota_limit: u64,
    pub votes: Vec<Hash>,
    pub proposer: Address,
}

impl BlockHeader {
    /// Calculate the block header hash. To maintain consistency we use RLP serialization.
    pub fn hash(&self) -> Hash {
        let rlp_data = rlp::encode(self);
        Hash::digest(&rlp_data)
    }
}

/// Structure encodable to RLP
impl Encodable for BlockHeader {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.prevhash);
        s.append(&self.timestamp);
        s.append(&self.height);
        s.append(&self.transactions_root);
        s.append(&self.state_root);
        s.append(&self.receipts_root);
        s.append(&self.logs_bloom.as_ref());
        s.append(&self.quota_used);
        s.append(&self.quota_limit);
        s.append_list(&self.votes);
        s.append(&self.proposer);
    }
}

#[derive(Default, Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub tx_hashes: Vec<Hash>,
    pub hash: Hash,
}

// TODO: proof
#[derive(Default, Debug, Clone)]
pub struct Proposal {
    pub prevhash: Hash,
    pub timestamp: u64,
    pub height: u64,
    pub quota_limit: u64,
    pub proposer: Address,
    pub tx_hashes: Vec<Hash>,
}
