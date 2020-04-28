use std::collections::BTreeMap;

use muta_codec_derive::RlpFixedCodec;
use serde::{Deserialize, Serialize};

use binding_macro::{SchemaEvent, SchemaObject};
use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::traits::SchemaGenerator;
use protocol::types::{Address, Bytes, Hash};
use protocol::ProtocolResult;

/// Payload
#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug)]
pub struct InitGenesisPayload {
    pub id:     Hash,
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
    pub issuer: Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct CreateAssetPayload {
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct GetAssetPayload {
    pub id: Hash,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct TransferPayload {
    pub asset_id: Hash,
    pub to:       Address,
    pub value:    u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct TransferEvent {
    pub asset_id: Hash,
    pub from:     Address,
    pub to:       Address,
    pub value:    u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct ApprovePayload {
    pub asset_id: Hash,
    pub to:       Address,
    pub value:    u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct ApproveEvent {
    pub asset_id: Hash,
    pub grantor:  Address,
    pub grantee:  Address,
    pub value:    u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct TransferFromPayload {
    pub asset_id:  Hash,
    pub sender:    Address,
    pub recipient: Address,
    pub value:     u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct TransferFromEvent {
    pub asset_id:  Hash,
    pub caller:    Address,
    pub sender:    Address,
    pub recipient: Address,
    pub value:     u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct GetBalancePayload {
    pub asset_id: Hash,
    pub user:     Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default, SchemaObject)]
pub struct GetBalanceResponse {
    pub asset_id: Hash,
    pub user:     Address,
    pub balance:  u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct GetAllowancePayload {
    pub asset_id: Hash,
    pub grantor:  Address,
    pub grantee:  Address,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, Default, SchemaObject)]
pub struct GetAllowanceResponse {
    pub asset_id: Hash,
    pub grantor:  Address,
    pub grantee:  Address,
    pub value:    u64,
}

#[derive(RlpFixedCodec, Deserialize, Serialize, Clone, Debug, PartialEq, Default, SchemaObject)]
pub struct Asset {
    pub id:     Hash,
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
    pub issuer: Address,
}

pub struct AssetBalance {
    pub value:     u64,
    pub allowance: BTreeMap<Address, u64>,
}

#[derive(RlpFixedCodec)]
struct AllowanceCodec {
    pub addr:  Address,
    pub total: u64,
}

#[derive(SchemaEvent)]
pub enum Event {
    Transfer(TransferEvent),
    Approve(ApproveEvent),
    TransferFrom(TransferFromEvent),
}

impl rlp::Decodable for AssetBalance {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let value = rlp.at(0)?.as_val()?;
        let codec_list: Vec<AllowanceCodec> = rlp::decode_list(rlp.at(1)?.as_raw());
        let mut allowance = BTreeMap::new();
        for v in codec_list {
            allowance.insert(v.addr, v.total);
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
            let fixed_codec = AllowanceCodec {
                addr:  address.clone(),
                total: *allowance,
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
