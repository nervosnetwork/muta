use serde::{Deserialize, Serialize};

use bytes::Bytes;

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Hash};
use protocol::ProtocolResult;

/// Payload
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

/// Response
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GetBalanceResponse {
    pub asset_id: Hash,
    pub balance:  u64,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Asset {
    pub id:     Hash,
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
    pub owner:  Address,
}

impl rlp::Decodable for Asset {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self {
            id:     rlp.at(0)?.as_val()?,
            name:   rlp.at(1)?.as_val()?,
            symbol: rlp.at(2)?.as_val()?,
            supply: rlp.at(3)?.as_val()?,
            owner:  rlp.at(4)?.as_val()?,
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
            .append(&self.owner);
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
