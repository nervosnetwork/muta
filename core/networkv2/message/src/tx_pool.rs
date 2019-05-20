use prost::Message as ProstMessage;

use core_serialization::{CodecError, SignedTransaction as SerSignedTransaction};
use core_types::SignedTransaction;

use crate::common::des_ser_sig_txs;

#[derive(Clone, PartialEq, ProstMessage)]
pub struct BroadcastTxs {
    #[prost(message, repeated, tag = "1")]
    pub txs: Vec<SerSignedTransaction>,
}

impl BroadcastTxs {
    pub fn from(txs: Vec<SignedTransaction>) -> Self {
        let txs = txs.into_iter().map(Into::into).collect::<Vec<_>>();

        BroadcastTxs { txs }
    }

    pub fn des(self) -> Result<Vec<SignedTransaction>, CodecError> {
        des_ser_sig_txs(self.txs)
    }
}
