use muta_codec_derive::RlpFixedCodec;
use serde::{Deserialize, Serialize};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Bytes};
use protocol::ProtocolResult;

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct InitGenesisPayload {
    pub admin:                  Address,
    pub register_service_names: Vec<String>,
    pub verified_method_names:  Vec<String>,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct AddVerifiedItemPayload {
    pub service_name: String,
    pub method_name:  String,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct RemoveVerifiedItemPayload {
    pub service_name: String,
}
