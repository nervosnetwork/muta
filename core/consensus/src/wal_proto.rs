use std::convert::TryFrom;

use prost::Message;

use protocol::codec::{transaction, ProtocolCodecSync};
use protocol::types::SignedTransaction;
use protocol::{Bytes, ProtocolError, ProtocolResult};

use crate::{fixed_types, ConsensusError, ConsensusType};

#[derive(Clone, Message)]
pub struct FixedSignedTxs {
    #[prost(message, repeated, tag = "1")]
    pub inner: Vec<transaction::SignedTransaction>,
}

impl From<fixed_types::FixedSignedTxs> for FixedSignedTxs {
    fn from(txs: fixed_types::FixedSignedTxs) -> FixedSignedTxs {
        let inner = txs
            .inner
            .into_iter()
            .map(transaction::SignedTransaction::from)
            .collect::<Vec<_>>();
        FixedSignedTxs { inner }
    }
}

impl TryFrom<FixedSignedTxs> for fixed_types::FixedSignedTxs {
    type Error = ProtocolError;

    fn try_from(txs: FixedSignedTxs) -> Result<fixed_types::FixedSignedTxs, Self::Error> {
        let mut inner = Vec::new();
        for tx in txs.inner.into_iter() {
            let tmp = SignedTransaction::try_from(tx)?;
            inner.push(tmp);
        }

        Ok(fixed_types::FixedSignedTxs { inner })
    }
}

impl ProtocolCodecSync for fixed_types::FixedSignedTxs {
    fn encode_sync(&self) -> ProtocolResult<Bytes> {
        let ser_type = FixedSignedTxs::from(self.clone());
        let mut buf = Vec::with_capacity(ser_type.encoded_len());

        ser_type
            .encode(&mut buf)
            .map_err(|_| ConsensusError::EncodeErr(ConsensusType::WALSignedTxs))?;
        Ok(Bytes::from(buf))
    }

    fn decode_sync(data: Bytes) -> ProtocolResult<Self> {
        let ser_type = FixedSignedTxs::decode(data)
            .map_err(|_| ConsensusError::DecodeErr(ConsensusType::WALSignedTxs))?;

        fixed_types::FixedSignedTxs::try_from(ser_type)
    }
}
