use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use binding_macro::SchemaObject;

use protocol::traits::SchemaGenerator;
use protocol::types::{Hash, Hex};

#[derive(Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct KeccakPayload {
    pub hex_str: Hex,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, SchemaObject)]
pub struct KeccakResponse {
    pub result: Hash,
}

#[derive(Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct SigVerifyPayload {
    pub hash:    Hash,
    pub sig:     Hex,
    pub pub_key: Hex,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, SchemaObject)]
pub struct SigVerifyResponse {
    pub is_ok: bool,
}
