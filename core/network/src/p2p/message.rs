pub mod consensus;
pub mod synchronizer;
pub mod tx_pool;

use prost::Message as ProstMessage;

use crate::Message as NetworkMessage;

// use sub-mod message types
use consensus::ConsensusMessage;
use packed_message::Message as P2PMessage;
use synchronizer::SynchronizerMessage;
use tx_pool::TxPoolMessage;

// re-export
pub use packed_message::Message;

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PackedMessage {
    #[prost(oneof = "Message", tags = "1, 2, 3")]
    pub message: Option<Message>,
}

pub mod packed_message {
    use super::{ConsensusMessage, SynchronizerMessage, TxPoolMessage};

    use prost::Oneof;

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        TxPoolMessage(TxPoolMessage),
        #[prost(message, tag = "2")]
        SynchronizerMessage(SynchronizerMessage),
        #[prost(message, tag = "3")]
        ConsensusMessage(ConsensusMessage),
    }
}

// Conversion from core-types to core-serialization
impl From<NetworkMessage> for P2PMessage {
    fn from(msg: NetworkMessage) -> P2PMessage {
        match msg {
            NetworkMessage::BroadcastTxs { txs } => {
                P2PMessage::TxPoolMessage(TxPoolMessage::broadcast_txs(txs))
            }
            NetworkMessage::PullTxs { uuid, hashes } => {
                P2PMessage::TxPoolMessage(TxPoolMessage::pull_txs(uuid, hashes))
            }
            NetworkMessage::BroadcastStatus { status } => {
                P2PMessage::SynchronizerMessage(SynchronizerMessage::broadcast_status(status))
            }
            NetworkMessage::PullBlocks { uuid, heights } => {
                P2PMessage::SynchronizerMessage(SynchronizerMessage::pull_blocks(uuid, heights))
            }
            NetworkMessage::PullTxsSync { uuid, hashes } => {
                P2PMessage::SynchronizerMessage(SynchronizerMessage::pull_txs_sync(uuid, hashes))
            }
            NetworkMessage::BroadcastPrposal { msg } => {
                P2PMessage::ConsensusMessage(ConsensusMessage::proposal(msg))
            }
            NetworkMessage::BroadcastVote { msg } => {
                P2PMessage::ConsensusMessage(ConsensusMessage::vote(msg))
            }
        }
    }
}
