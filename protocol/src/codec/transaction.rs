use std::convert::TryFrom;

use bytes::Bytes;
use prost::Message;

use crate::{
    codec::{primitive::Hash, CodecError, ProtocolCodecSync},
    field, impl_default_bytes_codec_for,
    types::primitive as protocol_primitive,
    ProtocolError, ProtocolResult,
};

#[derive(Clone, Message)]
pub struct TransactionRequest {
    #[prost(bytes, tag = "1")]
    pub service_name: Vec<u8>,

    #[prost(bytes, tag = "2")]
    pub method: Vec<u8>,

    #[prost(bytes, tag = "3")]
    pub payload: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct RawTransaction {
    #[prost(message, tag = "1")]
    pub chain_id: Option<Hash>,

    #[prost(message, tag = "2")]
    pub nonce: Option<Hash>,

    #[prost(uint64, tag = "3")]
    pub timeout: u64,

    #[prost(uint64, tag = "4")]
    pub cycles_limit: u64,

    #[prost(message, tag = "5")]
    pub request: Option<TransactionRequest>,
}

#[derive(Clone, Message)]
pub struct SignedTransaction {
    #[prost(message, tag = "1")]
    pub raw: Option<RawTransaction>,

    #[prost(message, tag = "2")]
    pub tx_hash: Option<Hash>,

    #[prost(bytes, tag = "3")]
    pub pubkey: Vec<u8>,

    #[prost(bytes, tag = "4")]
    pub signature: Vec<u8>,
}

// #################
// Conversion
// #################

// TransactionAction

impl From<transaction::TransactionRequest> for TransactionRequest {
    fn from(request: transaction::TransactionRequest) -> TransactionRequest {
        TransactionRequest {
            service_name: request.service_name.as_bytes().to_vec(),
            method:       request.method.as_bytes().to_vec(),
            payload:      request.payload.as_bytes().to_vec(),
        }
    }
}

impl TryFrom<TransactionRequest> for transaction::TransactionRequest {
    type Error = ProtocolError;

    fn try_from(
        request: TransactionRequest,
    ) -> Result<transaction::TransactionRequest, Self::Error> {
        Ok(transaction::TransactionRequest {
            service_name: String::from_utf8(request.service_name)
                .map_err(CodecError::FromStringUtf8)?,
            method:       String::from_utf8(request.method).map_err(CodecError::FromStringUtf8)?,
            payload:      String::from_utf8(request.payload).map_err(CodecError::FromStringUtf8)?,
        })
    }
}

// RawTransaction

impl From<transaction::RawTransaction> for RawTransaction {
    fn from(raw: transaction::RawTransaction) -> RawTransaction {
        let chain_id = Some(Hash::from(raw.chain_id));
        let nonce = Some(Hash::from(raw.nonce));
        let request = Some(TransactionRequest::from(raw.request));

        RawTransaction {
            chain_id,
            nonce,
            timeout: raw.timeout,
            cycles_limit: raw.cycles_limit,
            request,
        }
    }
}

impl TryFrom<RawTransaction> for transaction::RawTransaction {
    type Error = ProtocolError;

    fn try_from(raw: RawTransaction) -> Result<transaction::RawTransaction, Self::Error> {
        let chain_id = field!(raw.chain_id, "RawTransaction", "chain_id")?;
        let nonce = field!(raw.nonce, "RawTransaction", "nonce")?;
        let request = field!(raw.request, "RawTransaction", "request")?;

        let raw_tx = transaction::RawTransaction {
            chain_id:     protocol_primitive::Hash::try_from(chain_id)?,
            nonce:        protocol_primitive::Hash::try_from(nonce)?,
            timeout:      raw.timeout,
            cycles_limit: raw.cycles_limit,
            request:      transaction::TransactionRequest::try_from(request)?,
        };

        Ok(raw_tx)
    }
}

// SignedTransaction

impl From<transaction::SignedTransaction> for SignedTransaction {
    fn from(stx: transaction::SignedTransaction) -> SignedTransaction {
        let raw = RawTransaction::from(stx.raw);
        let tx_hash = Hash::from(stx.tx_hash);

        SignedTransaction {
            raw:       Some(raw),
            tx_hash:   Some(tx_hash),
            pubkey:    stx.pubkey.to_vec(),
            signature: stx.signature.to_vec(),
        }
    }
}

impl TryFrom<SignedTransaction> for transaction::SignedTransaction {
    type Error = ProtocolError;

    fn try_from(stx: SignedTransaction) -> Result<transaction::SignedTransaction, Self::Error> {
        let raw = field!(stx.raw, "SignedTransaction", "raw")?;
        let tx_hash = field!(stx.tx_hash, "SignedTransaction", "tx_hash")?;

        let stx = transaction::SignedTransaction {
            raw:       transaction::RawTransaction::try_from(raw)?,
            tx_hash:   protocol_primitive::Hash::try_from(tx_hash)?,
            pubkey:    Bytes::from(stx.pubkey),
            signature: Bytes::from(stx.signature),
        };

        Ok(stx)
    }
}

// #################
// Codec
// #################

impl_default_bytes_codec_for!(transaction, [RawTransaction, SignedTransaction]);
