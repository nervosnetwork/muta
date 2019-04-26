use prost::Message as ProstMessage;
use uuid::Uuid;

use core_serialization::SignedTransaction as SerSignedTransaction;
use core_types::{Hash, SignedTransaction};

#[derive(Clone, PartialEq, ProstMessage)]
pub struct BroadcastTxs {
    #[prost(message, repeated, tag = "1")]
    pub txs: Vec<SerSignedTransaction>,
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PullTxs {
    #[prost(string, tag = "1")]
    pub uuid: String,
    #[prost(bytes, repeated, tag = "2")]
    pub hashes: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PushTxs {
    #[prost(string, tag = "1")]
    pub uuid: String,
    #[prost(message, repeated, tag = "2")]
    pub sig_txs: Vec<SerSignedTransaction>,
}

pub mod packed_message {
    use prost::Oneof;

    use super::{BroadcastTxs, PullTxs, PushTxs};

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        BroadcastTxs(BroadcastTxs),

        #[prost(message, tag = "2")]
        PullTxs(PullTxs),

        #[prost(message, tag = "3")]
        PushTxs(PushTxs),
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct TxPoolMessage {
    #[prost(oneof = "packed_message::Message", tags = "1, 2, 3")]
    pub message: Option<packed_message::Message>,
}

impl TxPoolMessage {
    pub fn broadcast_txs(txs: Vec<SignedTransaction>) -> Self {
        let ser_txs = txs.into_iter().map(Into::into).collect::<Vec<_>>();
        let broadcast_txs = BroadcastTxs { txs: ser_txs };
        TxPoolMessage {
            message: Some(packed_message::Message::BroadcastTxs(broadcast_txs)),
        }
    }

    pub fn pull_txs(uuid: Uuid, hashes: Vec<Hash>) -> Self {
        let hashes = hashes
            .into_iter()
            .map(|h| h.as_bytes().to_vec())
            .collect::<_>();

        let pull_txs = PullTxs {
            uuid: uuid.to_string(),
            hashes,
        };

        TxPoolMessage {
            message: Some(packed_message::Message::PullTxs(pull_txs)),
        }
    }

    pub fn push_txs(uuid: String, sig_txs: Vec<SerSignedTransaction>) -> Self {
        let push_txs = PushTxs { uuid, sig_txs };

        TxPoolMessage {
            message: Some(packed_message::Message::PushTxs(push_txs)),
        }
    }
}
