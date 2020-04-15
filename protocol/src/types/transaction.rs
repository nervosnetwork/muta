use bytes::Bytes;
use fixed_codec_derive::RlpFixedCodec;

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::primitive::{Hash, JsonString};
use crate::ProtocolResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawTransaction {
    pub chain_id:     Hash,
    pub cycles_price: u64,
    pub cycles_limit: u64,
    pub nonce:        Hash,
    pub request:      TransactionRequest,
    pub timeout:      u64,
}

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct TransactionRequest {
    pub method:       String,
    pub service_name: String,
    pub payload:      JsonString,
}

#[derive(RlpFixedCodec, Clone, Debug, PartialEq, Eq)]
pub struct SignedTransaction {
    pub raw:       RawTransaction,
    pub tx_hash:   Hash,
    pub pubkey:    Bytes,
    pub signature: Bytes,
}
