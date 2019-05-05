use rlp::{Encodable, RlpStream};

use core_merkle::Proof;
use core_types::{Hash, Receipt};

#[derive(Debug, Clone)]
pub struct ReceiptProof {
    pub receipt: Receipt,
    pub merkle_proof: Proof<Hash>,
    pub block_number: u64,
}

/// Structure encodable to RLP
impl Encodable for ReceiptProof {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3);
        s.append(&self.receipt);
        s.append(&self.merkle_proof);
        s.append(&self.block_number);
    }
}
