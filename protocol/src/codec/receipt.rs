use std::convert::TryFrom;

use bytes::Bytes;
use prost::Message;

use crate::{
    codec::{primitive::Hash, CodecError, ProtocolCodecSync},
    field, impl_default_bytes_codec_for,
    traits::ServiceResponse,
    types::primitive as protocol_primitive,
    types::receipt as protocol_receipt,
    ProtocolError, ProtocolResult,
};

// #####################
// Protobuf
// #####################

#[derive(Clone, Message)]
pub struct Receipt {
    #[prost(message, tag = "1")]
    pub state_root: Option<Hash>,

    #[prost(uint64, tag = "2")]
    pub height: u64,

    #[prost(message, tag = "3")]
    pub tx_hash: Option<Hash>,

    #[prost(uint64, tag = "4")]
    pub cycles_used: u64,

    #[prost(message, repeated, tag = "5")]
    pub events: Vec<Event>,

    #[prost(message, tag = "6")]
    pub response: Option<ReceiptResponse>,
}

#[derive(Clone, Message)]
pub struct ReceiptResponse {
    #[prost(bytes, tag = "1")]
    pub service_name: Vec<u8>,

    #[prost(bytes, tag = "2")]
    pub method: Vec<u8>,

    #[prost(uint64, tag = "3")]
    pub code: u64,

    #[prost(bytes, tag = "4")]
    pub succeed_data: Vec<u8>,

    #[prost(bytes, tag = "5")]
    pub error_message: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct Event {
    #[prost(bytes, tag = "1")]
    pub service: Vec<u8>,
    #[prost(bytes, tag = "2")]
    pub topic:   Vec<u8>,

    #[prost(bytes, tag = "3")]
    pub data: Vec<u8>,
}

// #################
// Conversion
// #################

// ReceiptResult

impl From<receipt::ReceiptResponse> for ReceiptResponse {
    fn from(response: receipt::ReceiptResponse) -> ReceiptResponse {
        ReceiptResponse {
            service_name:  response.service_name.as_bytes().to_vec(),
            method:        response.method.as_bytes().to_vec(),
            code:          response.response.code,
            succeed_data:  response.response.succeed_data.as_bytes().to_vec(),
            error_message: response.response.error_message.as_bytes().to_vec(),
        }
    }
}

impl TryFrom<ReceiptResponse> for receipt::ReceiptResponse {
    type Error = ProtocolError;

    fn try_from(response: ReceiptResponse) -> Result<receipt::ReceiptResponse, Self::Error> {
        Ok(receipt::ReceiptResponse {
            service_name: String::from_utf8(response.service_name)
                .map_err(CodecError::FromStringUtf8)?,
            method:       String::from_utf8(response.method).map_err(CodecError::FromStringUtf8)?,
            response:     ServiceResponse {
                code:          response.code,
                succeed_data:  String::from_utf8(response.succeed_data)
                    .map_err(CodecError::FromStringUtf8)?,
                error_message: String::from_utf8(response.error_message)
                    .map_err(CodecError::FromStringUtf8)?,
            },
        })
    }
}

// Receipt

impl From<receipt::Receipt> for Receipt {
    fn from(receipt: receipt::Receipt) -> Receipt {
        let state_root = Some(Hash::from(receipt.state_root));
        let tx_hash = Some(Hash::from(receipt.tx_hash));
        let events = receipt.events.into_iter().map(Event::from).collect();
        let response = Some(ReceiptResponse::from(receipt.response));

        Receipt {
            state_root,
            height: receipt.height,
            tx_hash,
            cycles_used: receipt.cycles_used,
            events,
            response,
        }
    }
}

impl TryFrom<Receipt> for receipt::Receipt {
    type Error = ProtocolError;

    fn try_from(receipt: Receipt) -> Result<receipt::Receipt, Self::Error> {
        let state_root = field!(receipt.state_root, "Receipt", "state_root")?;
        let tx_hash = field!(receipt.tx_hash, "Receipt", "tx_hash")?;
        let response = field!(receipt.response, "Receipt", "response")?;
        let events = receipt
            .events
            .into_iter()
            .map(protocol_receipt::Event::try_from)
            .collect::<Result<Vec<protocol_receipt::Event>, ProtocolError>>()?;

        let receipt = receipt::Receipt {
            state_root: protocol_primitive::Hash::try_from(state_root)?,
            height: receipt.height,
            tx_hash: protocol_primitive::Hash::try_from(tx_hash)?,
            cycles_used: receipt.cycles_used,
            events,
            response: receipt::ReceiptResponse::try_from(response)?,
        };

        Ok(receipt)
    }
}

// Event
impl From<receipt::Event> for Event {
    fn from(event: receipt::Event) -> Event {
        Event {
            service: event.service.as_bytes().to_vec(),
            topic:   event.topic.as_bytes().to_vec(),
            data:    event.data.as_bytes().to_vec(),
        }
    }
}

impl TryFrom<Event> for receipt::Event {
    type Error = ProtocolError;

    fn try_from(event: Event) -> Result<receipt::Event, Self::Error> {
        Ok(receipt::Event {
            service: String::from_utf8(event.service).map_err(CodecError::FromStringUtf8)?,
            topic:   String::from_utf8(event.topic).map_err(CodecError::FromStringUtf8)?,
            data:    String::from_utf8(event.data).map_err(CodecError::FromStringUtf8)?,
        })
    }
}

// #################
// Codec
// #################

impl_default_bytes_codec_for!(receipt, [Receipt]);
