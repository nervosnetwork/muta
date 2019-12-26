use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::receipt::{Event, Receipt, ReceiptResponse};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl FixedCodec trait for types
impl_default_fixed_codec_for!(receipt, [Receipt, ReceiptResponse]);

impl rlp::Encodable for Receipt {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(6);
        s.append(&self.cycles_used);
        s.append(&self.epoch_id);
        s.begin_list(self.events.len());
        for e in &self.events {
            s.append(e);
        }
        s.append(&self.response);
        s.append(&self.state_root);
        s.append(&self.tx_hash);
    }
}

impl rlp::Decodable for Receipt {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 6 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let cycles_used: u64 = r.at(0)?.as_val()?;
        let epoch_id = r.at(1)?.as_val()?;
        let events: Vec<Event> = r.at(2)?.as_list()?;
        let response: ReceiptResponse = rlp::decode(r.at(3)?.as_raw())?;
        let state_root = rlp::decode(r.at(4)?.as_raw())?;
        let tx_hash = rlp::decode(r.at(5)?.as_raw())?;

        Ok(Receipt {
            state_root,
            epoch_id,
            events,
            tx_hash,
            cycles_used,
            response,
        })
    }
}

impl rlp::Encodable for ReceiptResponse {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5)
            .append(&self.is_error)
            .append(&self.method)
            .append(&self.ret)
            .append(&self.service_name);
    }
}

impl rlp::Decodable for ReceiptResponse {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 4 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let is_error = r.at(0)?.as_val()?;
        let method = r.at(1)?.as_val()?;
        let ret = r.at(2)?.as_val()?;
        let service_name = r.at(3)?.as_val()?;

        Ok(ReceiptResponse {
            is_error,
            method,
            ret,
            service_name,
        })
    }
}

impl rlp::Decodable for Event {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let service = r.at(0)?.as_val()?;
        let data = r.at(1)?.as_val()?;

        Ok(Event { service, data })
    }
}

impl rlp::Encodable for Event {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2).append(&self.service).append(&self.data);
    }
}
