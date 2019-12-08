use crate::types::{Bloom, MerkleRoot, Receipt};

#[derive(Debug, Default, Clone)]
pub struct ExecutorResp {
    pub receipts:        Vec<Receipt>,
    pub all_cycles_used: u64,
    pub logs_bloom:      Bloom,
    pub state_root:      MerkleRoot,
}
