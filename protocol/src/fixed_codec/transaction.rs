use bytes::{Bytes, BytesMut};

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::primitive::Hash;
use crate::types::transaction::{RawTransaction, SignedTransaction, TransactionRequest};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl ProtocolFixedCodec trait for types
impl_default_fixed_codec_for!(transaction, [RawTransaction, SignedTransaction]);

impl rlp::Encodable for RawTransaction {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(8);
        s.append(&self.chain_id.as_bytes().to_vec());
        s.append(&self.cycles_limit);
        s.append(&self.cycles_price);
        s.append(&self.nonce.as_bytes().to_vec());
        s.append(&self.request.method);
        s.append(&self.request.service_name);
        s.append(&self.request.payload);
        s.append(&self.timeout);
    }
}

impl rlp::Decodable for RawTransaction {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let chain_id = Hash::from_bytes(Bytes::from(r.at(0)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        let cycles_limit: u64 = r.at(1)?.as_val()?;
        let cycles_price: u64 = r.at(2)?.as_val()?;

        let nonce = Hash::from_bytes(Bytes::from(r.at(3)?.data()?))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        let request = TransactionRequest {
            method:       r.at(4)?.as_val()?,
            service_name: r.at(5)?.as_val()?,
            payload:      r.at(6)?.as_val()?,
        };
        let timeout = r.at(7)?.as_val()?;

        Ok(Self {
            chain_id,
            cycles_price,
            cycles_limit,
            nonce,
            request,
            timeout,
        })
    }
}

impl rlp::Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(4)
            .append(&self.pubkey.to_vec())
            .append(&self.raw)
            .append(&self.signature.to_vec())
            .append(&self.tx_hash);
    }
}

impl rlp::Decodable for SignedTransaction {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 4 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let pubkey = BytesMut::from(r.at(0)?.data()?).freeze();
        let raw: RawTransaction = rlp::decode(r.at(1)?.as_raw())?;
        let signature = BytesMut::from(r.at(2)?.data()?).freeze();
        let tx_hash = rlp::decode(r.at(3)?.as_raw())?;

        Ok(SignedTransaction {
            raw,
            tx_hash,
            pubkey,
            signature,
        })
    }
}
