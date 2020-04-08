use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use common_crypto::{
    Secp256k1PublicKey, Secp256k1Signature,
};
use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::ProtocolResult;
use protocol::types::{Address, Hash, Hex};

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
    pub hash: Hash,
    pub sig: Hex,
    pub pub_key: Hex,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct SigVerifyResponse {
    pub is_ok: bool,
}
