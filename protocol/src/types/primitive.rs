#![allow(clippy::all)]
construct_uint! {
    pub struct Balance(4);
}

use bytes::Bytes;

use crate::types::TypesError;
use crate::ProtocolResult;

const HASH_LEN: usize = 32;
const ADDRESS_LEN: usize = 21;

#[derive(Clone, Debug)]
pub struct Hash([u8; HASH_LEN]);
#[derive(Clone, Debug)]
pub struct Address([u8; ADDRESS_LEN]);

impl Hash {
    pub fn from_bytes(bytes: Bytes) -> ProtocolResult<Self> {
        if bytes.len() != ADDRESS_LEN {
            return Err(TypesError::HashLengthMismatch {
                expect: HASH_LEN,
                real:   bytes.len(),
            }
            .into());
        }

        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&bytes);
        Ok(Self::from_fixed_bytes(out))
    }

    pub fn from_fixed_bytes(bytes: [u8; HASH_LEN]) -> Self {
        Self(bytes)
    }

    pub fn from_hex(s: &str) -> ProtocolResult<Self> {
        let s = clean_0x(s);
        let bytes = hex::decode(s).map_err(TypesError::from)?;

        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&bytes);
        Ok(Self::from_fixed_bytes(out))
    }

    pub fn as_bytes(&self) -> Bytes {
        Bytes::from(self.0.as_ref())
    }

    pub fn into_fixed_bytes(self) -> [u8; HASH_LEN] {
        self.0
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }
}

pub fn clean_0x(s: &str) -> &str {
    if s.starts_with("0x") {
        &s[2..]
    } else {
        s
    }
}
