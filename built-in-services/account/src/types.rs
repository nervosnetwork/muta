use bytes::Bytes;
use serde::{Deserialize, Serialize};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::traits::Witness;
use protocol::types::{Address, Hash, Hex, TypesError};
use protocol::ProtocolResult;

pub const ACCOUNT_TYPE_PUBLIC_KEY: u8 = 0;
pub const ACCOUNT_TYPE_MULTI_SIG: u8 = 1;
pub const MAX_PERMISSION_ACCOUNTS: u8 = 16;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct VerifyPayload {
    pub tx_hash: Hash,
    pub witness: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct VerifyResponse {
    pub address: Address,
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
    pub address: Address,
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

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct WitnessAdapter {
    pub pubkeys:        Vec<Hex>,
    pub signatures:     Vec<Hex>,
    pub signature_type: u8,
    pub sender:         Address,
}

impl Witness for WitnessAdapter {
    fn as_bytes(&self) -> ProtocolResult<Bytes> {
        match serde_json::to_vec(&self) {
            Ok(b) => Ok(Bytes::from(b)),
            Err(_) => Err(TypesError::InvalidWitness.into()),
        }
    }

    fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        serde_json::from_slice(bytes.as_ref()).map_err(|_| TypesError::InvalidWitness.into())
    }

    fn as_string(&self) -> ProtocolResult<String> {
        serde_json::to_string(&self).map_err(|_| TypesError::InvalidWitness.into())
    }

    fn from_string(s: &str) -> ProtocolResult<Self> {
        serde_json::from_str(s).map_err(|_| TypesError::InvalidWitness.into())
    }

    fn from_single_sig_hex(pub_key: String, sig: String) -> ProtocolResult<Self> {
        Ok(Self {
            pubkeys:        vec![Hex::from_string(pub_key)?],
            signatures:     vec![Hex::from_string(sig)?],
            signature_type: 0,
            sender:         Address::from_hex("0x0000000000000000000000000000000000000000")?,
        })
    }

    fn from_multi_sig_hex(
        sender: Address,
        pub_keys: Vec<String>,
        sigs: Vec<String>,
    ) -> ProtocolResult<Self> {
        if pub_keys.is_empty()
            || pub_keys.len() != sigs.len()
            || pub_keys.len() > MAX_PERMISSION_ACCOUNTS as usize
        {
            return Err(TypesError::InvalidWitness.into());
        }

        let mut pubkeys = Vec::<Hex>::new();
        let mut signatures = Vec::<Hex>::new();
        let size = pub_keys.len();
        pubkeys.reserve(size);
        signatures.reserve(size);

        for i in 0..size {
            pubkeys.push(Hex::from_string(pub_keys[i].clone())?);
            signatures.push(Hex::from_string(sigs[i].clone())?);
        }

        Ok(Self {
            pubkeys,
            signatures,
            signature_type: ACCOUNT_TYPE_MULTI_SIG,
            sender,
        })
    }
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
