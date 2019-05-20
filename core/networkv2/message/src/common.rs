use std::convert::TryInto;

use prost::Message as ProstMessage;

use core_serialization::{CodecError, SignedTransaction as SerSignedTransaction};
use core_types::{Hash, SignedTransaction};

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PullTxs {
    #[prost(uint64, tag = "1")]
    pub uid: u64,
    #[prost(bytes, repeated, tag = "2")]
    pub hashes: Vec<Vec<u8>>,
}

impl PullTxs {
    pub fn from(uid: u64, hashes: Vec<Hash>) -> Self {
        let hashes = hashes
            .into_iter()
            .map(|h| h.as_bytes().to_vec())
            .collect::<Vec<_>>();

        PullTxs { uid, hashes }
    }

    pub fn des(self) -> Result<Vec<Hash>, CodecError> {
        let hashes = self
            .hashes
            .into_iter()
            .map(|h| Hash::from_bytes(h.as_slice()))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(hashes)
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct PushTxs {
    #[prost(uint64, tag = "1")]
    pub uid: u64,
    #[prost(message, repeated, tag = "2")]
    pub sig_txs: Vec<SerSignedTransaction>,
}

impl PushTxs {
    pub fn from(uid: u64, txs: Vec<SignedTransaction>) -> Self {
        let sig_txs = txs.into_iter().map(Into::into).collect::<Vec<_>>();

        PushTxs { uid, sig_txs }
    }

    pub fn des(self) -> Result<Vec<SignedTransaction>, CodecError> {
        des_ser_sig_txs(self.sig_txs)
    }
}

pub fn des_ser_sig_txs(
    txs: Vec<SerSignedTransaction>,
) -> Result<Vec<SignedTransaction>, CodecError> {
    txs.into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, CodecError>>()
}
