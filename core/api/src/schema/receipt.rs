use crate::schema::{Hash, MerkleRoot, Uint64};

#[derive(juniper::GraphQLObject, Clone)]
pub struct Receipt {
    pub state_root:  MerkleRoot,
    pub height:      Uint64,
    pub tx_hash:     Hash,
    pub cycles_used: Uint64,
    pub events:      Vec<Event>,
    pub response:    ReceiptResponse,
}

#[derive(juniper::GraphQLObject, Clone)]
pub struct Event {
    pub service: String,
    pub data:    String,
}

#[derive(juniper::GraphQLObject, Clone)]
pub struct ReceiptResponse {
    pub service_name: String,
    pub method:       String,
    pub ret:          String,
    pub is_error:     bool,
}

impl From<protocol::types::Receipt> for Receipt {
    fn from(receipt: protocol::types::Receipt) -> Self {
        Self {
            state_root:  MerkleRoot::from(receipt.state_root),
            height:      Uint64::from(receipt.height),
            tx_hash:     Hash::from(receipt.tx_hash),
            cycles_used: Uint64::from(receipt.cycles_used),
            events:      receipt.events.into_iter().map(Event::from).collect(),
            response:    ReceiptResponse::from(receipt.response),
        }
    }
}

impl From<protocol::types::Event> for Event {
    fn from(event: protocol::types::Event) -> Self {
        Self {
            service: event.service,
            data:    event.data,
        }
    }
}

impl From<protocol::types::ReceiptResponse> for ReceiptResponse {
    fn from(response: protocol::types::ReceiptResponse) -> Self {
        Self {
            service_name: response.service_name,
            method:       response.method,
            ret:          response.ret,
            is_error:     response.is_error,
        }
    }
}
