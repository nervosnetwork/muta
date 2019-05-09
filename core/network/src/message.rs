use uuid::Uuid;

use core_consensus::Status;
use core_types::{Hash, SignedTransaction};

pub enum Message {
    BroadcastTxs { txs: Vec<SignedTransaction> },
    PullTxs { uuid: Uuid, hashes: Vec<Hash> },

    BroadcastStatus { status: Status },
    PullBlocks { uuid: Uuid, heights: Vec<u64> },
    PullTxsSync { uuid: Uuid, hashes: Vec<Hash> },

    BroadcastPrposal { msg: Vec<u8> },
    BroadcastVote { msg: Vec<u8> },
}
