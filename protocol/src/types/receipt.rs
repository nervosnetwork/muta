use crate::types::{Hash, MerkleRoot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub service: String,
    pub data:    String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub state_root:  MerkleRoot,
    pub height:      u64,
    pub tx_hash:     Hash,
    pub cycles_used: u64,
    pub events:      Vec<Event>,
    pub response:    ReceiptResponse,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiptResponse {
    pub service_name: String,
    pub method:       String,
    pub ret:          String,
    pub is_error:     bool,
}
