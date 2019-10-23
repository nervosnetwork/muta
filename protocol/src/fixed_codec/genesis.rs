use bytes::Bytes;

use crate::fixed_codec::{FixedCodecError, ProtocolFixedCodec};
use crate::types::genesis::{Genesis, GenesisStateAlloc, GenesisStateAsset, GenesisSystemToken};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

impl_default_fixed_codec_for!(genesis, [
    Genesis,
    GenesisStateAlloc,
    GenesisStateAsset,
    GenesisSystemToken
]);

impl rlp::Encodable for GenesisSystemToken {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(4)
            .append(&self.code.as_bytes())
            .append(&self.name.as_bytes())
            .append(&self.supply)
            .append(&self.symbol.as_bytes());
    }
}

impl rlp::Decodable for GenesisSystemToken {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 4 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let mut values = Vec::with_capacity(4);

        for val in r {
            let data = val.data()?;
            values.push(data)
        }

        let code = String::from_utf8(values[0].to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let name = String::from_utf8(values[1].to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let supply: u64 = r.at(2)?.as_val()?;
        let symbol = String::from_utf8(values[3].to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(GenesisSystemToken {
            code,
            name,
            supply,
            symbol,
        })
    }
}

impl rlp::Encodable for GenesisStateAsset {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append(&self.asset_id.as_bytes())
            .append(&self.balance.as_bytes());
    }
}

impl rlp::Decodable for GenesisStateAsset {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let asset_id = String::from_utf8(r.at(0)?.data()?.to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let balance = String::from_utf8(r.at(1)?.data()?.to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(GenesisStateAsset { asset_id, balance })
    }
}

impl rlp::Encodable for GenesisStateAlloc {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append(&self.address.as_bytes())
            .append_list(&self.assets);
    }
}

impl rlp::Decodable for GenesisStateAlloc {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let address = String::from_utf8(r.at(0)?.data()?.to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let assets: Vec<GenesisStateAsset> = rlp::decode_list(r.at(1)?.as_raw());

        Ok(GenesisStateAlloc { address, assets })
    }
}

impl rlp::Encodable for Genesis {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(4)
            .append(&self.prevhash.as_bytes())
            .append_list(&self.state_alloc)
            .append(&self.system_token)
            .append(&self.timestamp);
    }
}

impl rlp::Decodable for Genesis {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 4 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let prevhash = String::from_utf8(r.at(0)?.data()?.to_vec())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        let state_alloc: Vec<GenesisStateAlloc> = rlp::decode_list(r.at(1)?.as_raw());
        let system_token = rlp::decode(r.at(2)?.as_raw())?;
        let timestamp = r.at(3)?.as_val()?;

        Ok(Genesis {
            timestamp,
            prevhash,
            system_token,
            state_alloc,
        })
    }
}
