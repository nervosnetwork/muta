use serde::{Deserialize, Serialize};

use bytes::Bytes;
use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Hash, Hex};
use protocol::ProtocolResult;

pub const ACCOUNT_TYPE_PUBLIC_KEY: u8 = 0;
pub const ACCOUNT_TYPE_MULTI_SIG: u8 = 1;
pub const MAX_PERMISSION_ACCOUNTS: u8 = 16;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct VerifyPayload {
    pub hash:    Hash,
    pub sig:     Hex,
    pub pub_key: Hex,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct VerifyResponse {
    pub is_ok: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GetAccountPayload {
    pub user: Address,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GenerateAccountPayload {
    pub accounts:  Vec<PayloadAccount>,
    pub threshold: u8,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct GenerateAccountResponse {
    pub accounts:  Vec<PayloadAccount>,
    pub threshold: u8,
    pub address:   Address,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct PayloadAccount {
    pub address: Address,
    pub weight:  u8,
}

pub struct Permission {
    pub accounts:  Vec<Account>,
    pub threshold: u8,
}

pub struct Account {
    pub address:       Address,
    pub account_type:  u8,
    pub permission_id: u8,
    pub weight:        u8,
}

impl rlp::Encodable for Account {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(4)
            .append(&self.address)
            .append(&self.account_type)
            .append(&self.permission_id)
            .append(&self.weight);
    }
}

impl rlp::Decodable for Account {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Account {
            address:       rlp::decode(rlp.at(0)?.as_raw())?,
            account_type:  rlp.at(1)?.as_val()?,
            permission_id: rlp.at(2)?.as_val()?,
            weight:        rlp.at(3)?.as_val()?,
        })
    }
}

impl FixedCodec for Account {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}

impl rlp::Encodable for Permission {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2)
            .append_list(&self.accounts)
            .append(&self.threshold);
    }
}

impl rlp::Decodable for Permission {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Permission {
            accounts:  rlp::decode_list(rlp.at(0)?.as_raw()),
            threshold: rlp.at(1)?.as_val()?,
        })
    }
}

impl FixedCodec for Permission {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}
