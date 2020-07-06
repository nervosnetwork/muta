use muta_codec_derive::RlpFixedCodec;
use serde::{Deserialize, Serialize};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Bytes};
use protocol::ProtocolResult;

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct InitGenesisPayload {
    pub admin:          Address,
    pub verified_items: Vec<VerifiedItem>,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct VerifiedItem {
    pub service_name: String,
    pub method_name:  String,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct AddVerifiedItemPayload {
    pub service_name: String,
    pub method_name:  String,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct RemoveVerifiedItemPayload {
    pub service_name: String,
    pub method_name:  String,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct SetAdminPayload {
    pub new_admin: Address,
}

impl From<AddVerifiedItemPayload> for VerifiedItem {
    fn from(payload: AddVerifiedItemPayload) -> Self {
        VerifiedItem {
            service_name: payload.service_name,
            method_name:  payload.method_name,
        }
    }
}

impl From<RemoveVerifiedItemPayload> for VerifiedItem {
    fn from(payload: RemoveVerifiedItemPayload) -> Self {
        VerifiedItem {
            service_name: payload.service_name,
            method_name:  payload.method_name,
        }
    }
}
