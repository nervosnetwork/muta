use serde::Serialize;

use core_types::{self, Hash};

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct TxResponse {
    pub hash:   Hash,
    pub status: String,
}

impl TxResponse {
    pub fn new(hash: Hash, status: String) -> Self {
        TxResponse { hash, status }
    }
}
