use uuid::Uuid;

use core_types::{Hash, SignedTransaction};

pub enum Message {
    BroadcastTxs { txs: Vec<SignedTransaction> },
    PullTxs { uuid: Uuid, hashes: Vec<Hash> },
}
