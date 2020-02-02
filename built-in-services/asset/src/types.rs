use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use bytes::Bytes;

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Hash};
use protocol::ProtocolResult;

/// Payload
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct InitGenesisPayload {
    pub id:     Hash,
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
    pub issuer: Address,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CreateAssetPayload {
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GetAssetPayload {
    pub id: Hash,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TransferPayload {
    pub asset_id: Hash,
    pub to:       Address,
    pub value:    u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GetBalancePayload {
    pub asset_id: Hash,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GetBalanceResponse {
    pub asset_id: Hash,
    pub balance:  u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GetAllowancePayload {
    pub asset_id: Hash,
    pub grantee:  Address,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GetAllowanceResponse {
    pub asset_id: Hash,
    pub grantee:  Address,
    pub total:    u64,
    pub used:     u64,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Asset {
    pub id:     Hash,
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
    pub issuer: Address,
}

pub struct AssetBalance {
    pub value:     u64,
    pub allowance: BTreeMap<Address, Approval>,
}

pub struct Approval {
    pub total: u64,
    pub used:  u64,
}

struct AssetBalanceCodec {
    pub addr:  Address,
    pub total: u64,
    pub used:  u64,
}

impl rlp::Decodable for Asset {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self {
            id:     rlp.at(0)?.as_val()?,
            name:   rlp.at(1)?.as_val()?,
            symbol: rlp.at(2)?.as_val()?,
            supply: rlp.at(3)?.as_val()?,
            issuer: rlp.at(4)?.as_val()?,
        })
    }
}

impl rlp::Encodable for Asset {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5)
            .append(&self.id)
            .append(&self.name)
            .append(&self.symbol)
            .append(&self.supply)
            .append(&self.issuer);
    }
}

impl FixedCodec for Asset {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}

impl rlp::Decodable for AssetBalanceCodec {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self {
            addr:  rlp.at(0)?.as_val()?,
            total: rlp.at(1)?.as_val()?,
            used:  rlp.at(2)?.as_val()?,
        })
    }
}

impl rlp::Encodable for AssetBalanceCodec {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3)
            .append(&self.addr)
            .append(&self.total)
            .append(&self.used);
    }
}

impl rlp::Decodable for AssetBalance {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let value = rlp.at(0)?.as_val()?;
        let codec_list: Vec<AssetBalanceCodec> = rlp::decode_list(rlp.at(1)?.as_raw());
        let mut allowance = BTreeMap::new();
        for v in codec_list {
            allowance.insert(v.addr, Approval {
                total: v.total,
                used:  v.used,
            });
        }

        Ok(AssetBalance { value, allowance })
    }
}

impl rlp::Encodable for AssetBalance {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2);
        s.append(&self.value);

        let mut codec_list = Vec::with_capacity(self.allowance.len());

        for (address, allowance) in self.allowance.iter() {
            let fixed_codec = AssetBalanceCodec {
                addr:  address.clone(),
                total: allowance.total,
                used:  allowance.used,
            };

            codec_list.push(fixed_codec);
        }

        s.append_list(&codec_list);
    }
}

impl FixedCodec for AssetBalance {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}
