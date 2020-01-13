use serde::{Deserialize, Serialize};

use bytes::Bytes;

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::types::{Address, Hash};
use protocol::ProtocolResult;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DeployPayload {
    pub code:      Bytes,
    pub init_args: Bytes,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ExecPayload {
    pub address: Address,
    pub args:    Bytes,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ExecResp {
    pub ret:      String,
    pub is_error: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct InterpreterResult {
    pub cycles_used: u64,
    pub ret:         Bytes,
    pub ret_code:    i8,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Contract {
    pub code_hash: Hash,
}

impl FixedCodec for Contract {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(self.code_hash.as_bytes())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(Self {
            code_hash: Hash::from_bytes(bytes)?,
        })
    }
}
