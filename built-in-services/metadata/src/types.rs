use serde::{Deserialize, Serialize};

use protocol::types::{Address, Metadata, Validator};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct InitGenesisPayload {
    pub admin:    Address,
    pub metadata: Metadata,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UpdateMetadataPayload {
    pub verifier_list:   Vec<Validator>,
    pub interval:        u64,
    pub propose_ratio:   u64,
    pub prevote_ratio:   u64,
    pub precommit_ratio: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UpdateRatioPayload {
    pub propose_ratio:   u64,
    pub prevote_ratio:   u64,
    pub precommit_ratio: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UpdateValidatorsPayload {
    pub verifier_list: Vec<Validator>,
}
