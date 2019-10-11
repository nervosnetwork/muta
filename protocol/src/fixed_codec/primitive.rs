use bytes::Bytes;

use crate::fixed_codec::{FixedCodecError, ProtocolFixedCodec};
use crate::types::{Asset, AssetID, Balance, ContractAddress, Hash};
use crate::ProtocolResult;

impl ProtocolFixedCodec for AssetID {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(self.as_bytes())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        AssetID::from_bytes(bytes)
    }
}

impl ProtocolFixedCodec for Asset {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}

impl rlp::Encodable for Asset {
    /// Append a value to the stream
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(6);
        s.append(&self.id.as_bytes().to_vec());
        s.append(&self.manage_contract.as_bytes().to_vec());
        s.append(&self.name.as_bytes());
        s.append(&self.storage_root.as_bytes().to_vec());
        s.append(&self.supply.to_bytes_be());
        s.append(&self.symbol.as_bytes());
    }
}

impl rlp::Decodable for Asset {
    /// Decode a value from RLP bytes
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 6 {
            return Err(rlp::DecoderError::RlpInvalidLength);
        }

        let mut values = Vec::with_capacity(6);

        for val in r {
            let data = val.data()?;
            values.push(data)
        }

        let id = Hash::from_bytes(Bytes::from(values[0]))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let manage_contract = ContractAddress::from_bytes(Bytes::from(values[1]))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let name = String::from_utf8(values[2].to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let storage_root = Hash::from_bytes(Bytes::from(values[3]))
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let supply = Balance::from_bytes_be(values[4]);
        let symbol = String::from_utf8(values[5].to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(Asset {
            id,
            manage_contract,
            name,
            storage_root,
            supply,
            symbol,
        })
    }
}
