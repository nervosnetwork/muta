use bytes::Bytes;
use fixed_codec_derive::RlpFixedCodec;

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::primitive::{Hash, JsonString};
use crate::ProtocolResult;

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct RawTransaction {
    pub chain_id:     Hash,
    pub nonce:        Hash,
    pub timeout:      u64,
    pub cycles_price: u64,
    pub cycles_limit: u64,
    pub request:      TransactionRequest,
}

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct TransactionRequest {
    pub service_name: String,
    pub method:       String,
    pub payload:      JsonString,
}

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct SignedTransaction {
    pub raw:       RawTransaction,
    pub tx_hash:   Hash,
    pub pubkey:    Bytes,
    pub signature: Bytes,
}
