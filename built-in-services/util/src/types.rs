use protocol::types::{Hash, Hex};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct KeccakPayload {
    pub hex_str: Hex,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct KeccakResponse {
    pub result: Hash,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SigVerifyPayload {
    pub hash:    Hash,
    pub sig:     Hex,
    pub pub_key: Hex,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct SigVerifyResponse {
    pub is_ok: bool,
}
