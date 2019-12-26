use std::mem;

use byteorder::{ByteOrder, LittleEndian};
use bytes::Bytes;

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::{Account, Address, Fee, Hash};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl ProtocolFixedCodec trait for types
impl_default_fixed_codec_for!(primitive, [Hash, Fee, Address, Account]);

impl FixedCodec for bool {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let bs = if *self {
            [1u8; mem::size_of::<u8>()]
        } else {
            [0u8; mem::size_of::<u8>()]
        };

        Ok(Bytes::from(bs.as_ref()))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        let u = *bytes.to_vec().get(0).ok_or(FixedCodecError::DecodeBool)?;

        match u {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(FixedCodecError::DecodeBool.into()),
        }
    }
}

impl FixedCodec for u8 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from([*self].as_ref()))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        let u = *bytes.to_vec().get(0).ok_or(FixedCodecError::DecodeUint8)?;

        Ok(u)
    }
}

impl FixedCodec for u32 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u32>()];
        LittleEndian::write_u32(&mut buf, *self);

        Ok(Bytes::from(buf.as_ref()))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u32(bytes.as_ref()))
    }
}

impl FixedCodec for u64 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut buf, *self);

        Ok(Bytes::from(buf.as_ref()))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u64(bytes.as_ref()))
    }
}

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

// AssetID, MerkleRoot are alias of Hash type
impl rlp::Encodable for Hash {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1).append(&self.as_bytes().to_vec());
    }
}

impl rlp::Decodable for Hash {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let hash = Hash::from_bytes(Bytes::from(r.at(0)?.data()?))
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
        let address = Address::from_bytes(Bytes::from(r.at(0)?.data()?))
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
