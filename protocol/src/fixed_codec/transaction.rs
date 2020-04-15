use bytes::BytesMut;

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::{Hash, RawTransaction, TransactionRequest};
use crate::ProtocolResult;

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
        let chain_id = Hash::from_bytes(BytesMut::from(r.at(0)?.data()?).freeze())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        let cycles_limit: u64 = r.at(1)?.as_val()?;
        let cycles_price: u64 = r.at(2)?.as_val()?;

        let nonce = Hash::from_bytes(BytesMut::from(r.at(3)?.data()?).freeze())
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

impl FixedCodec for RawTransaction {
    fn encode_fixed(&self) -> ProtocolResult<bytes::Bytes> {
        Ok(bytes::Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: bytes::Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}
