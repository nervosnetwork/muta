use std::mem;

use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use bytes::{Bytes, BytesMut};

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::{Account, Address, Fee, Hash};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl FixedCodec trait for types
impl_default_fixed_codec_for!(primitive, [Hash, Fee, Address, Account]);

impl FixedCodec for String {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(self.clone()))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        String::from_utf8(bytes.to_vec()).map_err(|e| FixedCodecError::StringUTF8(e).into())
    }
}

impl FixedCodec for Bytes {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(self.clone())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(bytes)
    }
}

impl FixedCodec for u64 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut bs = [0u8; mem::size_of::<u64>()];
        bs.as_mut()
            .write_u64::<LittleEndian>(*self)
            .expect("write u64 should not fail");

        Ok(BytesMut::from(bs.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u64(bytes.as_ref()))
    }
}

// AssetID, MerkleRoot are alias of Hash type
impl rlp::Encodable for Hash {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1).append(&self.as_bytes().to_vec());
    }
}

impl rlp::Decodable for Hash {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let hash = Hash::from_bytes(BytesMut::from(r.at(0)?.data()?).freeze())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        Ok(hash)
    }
}

impl rlp::Encodable for Fee {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(2).append(&self.asset_id).append(&self.cycle);
    }
}

impl rlp::Decodable for Fee {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if !r.is_list() && r.size() != 2 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let asset_id: Hash = rlp::decode(r.at(0)?.as_raw())?;
        let cycle = r.at(1)?.as_val()?;

        Ok(Fee { asset_id, cycle })
    }
}

impl rlp::Encodable for Address {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1).append(&self.as_bytes().to_vec());
    }
}

impl rlp::Decodable for Address {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let address = Address::from_bytes(BytesMut::from(r.at(0)?.data()?).freeze())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;

        Ok(address)
    }
}

impl rlp::Encodable for Account {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1);
        s.append(&self.storage_root);
    }
}

impl rlp::Decodable for Account {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let storage_root: Hash = r.at(0)?.as_val()?;
        Ok(Self { storage_root })
    }
}
