use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::traits::ServiceResponse;
use crate::types::receipt::ReceiptResponse;
use crate::ProtocolResult;

impl rlp::Encodable for ReceiptResponse {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5)
            .append(&self.response.code)
            .append(&self.response.succeed_data)
            .append(&self.response.error_message)
            .append(&self.method)
            .append(&self.service_name);
    }
}

impl rlp::Decodable for ReceiptResponse {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 5 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let code = r.at(0)?.as_val()?;
        let succeed_data = r.at(1)?.as_val()?;
        let error_message = r.at(2)?.as_val()?;
        let method = r.at(3)?.as_val()?;
        let service_name = r.at(4)?.as_val()?;

        Ok(ReceiptResponse {
            service_name,
            method,
            response: ServiceResponse {
                code,
                succeed_data,
                error_message,
            },
        })
    }
}

impl FixedCodec for ReceiptResponse {
    fn encode_fixed(&self) -> ProtocolResult<bytes::Bytes> {
        Ok(bytes::Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: bytes::Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}
