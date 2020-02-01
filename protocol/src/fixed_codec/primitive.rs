use std::mem;

use byteorder::{ByteOrder, LittleEndian};
use bytes::{Bytes, BytesMut};

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::{Address, Hash, Metadata};
use crate::{impl_default_fixed_codec_for, ProtocolResult};

// Impl FixedCodec trait for types
impl_default_fixed_codec_for!(primitive, [Hash, Address, Metadata]);

impl FixedCodec for bool {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let bs = if *self {
            [1u8; mem::size_of::<u8>()]
        } else {
            [0u8; mem::size_of::<u8>()]
        };

        Ok(BytesMut::from(bs.as_ref()).freeze())
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
        Ok(BytesMut::from([*self].as_ref()).freeze())
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

        Ok(BytesMut::from(buf.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u32(bytes.as_ref()))
    }
}

impl FixedCodec for u64 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut buf, *self);

        Ok(BytesMut::from(buf.as_ref()).freeze())
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
        let hash = Hash::from_bytes(BytesMut::from(r.at(0)?.data()?).freeze())
            .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
        Ok(hash)
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

impl rlp::Encodable for Metadata {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(10)
            .append(&self.chain_id)
            .append(&self.common_ref)
            .append(&self.timeout_gap)
            .append(&self.cycles_limit)
            .append(&self.cycles_price)
            .append(&self.interval)
            .append_list(&self.verifier_list)
            .append(&self.propose_ratio)
            .append(&self.prevote_ratio)
            .append(&self.precommit_ratio);
    }
}

impl rlp::Decodable for Metadata {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let chain_id: Hash = r.at(0)?.as_val()?;
        let common_ref: String = r.at(1)?.as_val()?;
        let timeout_gap: u64 = r.at(2)?.as_val()?;
        let cycles_limit: u64 = r.at(3)?.as_val()?;
        let cycles_price: u64 = r.at(4)?.as_val()?;
        let interval: u64 = r.at(5)?.as_val()?;
        let verifier_list: Vec<Address> = r.at(6)?.as_list()?;
        let propose_ratio: u64 = r.at(7)?.as_val()?;
        let prevote_ratio: u64 = r.at(8)?.as_val()?;
        let precommit_ratio: u64 = r.at(9)?.as_val()?;

        Ok(Self {
            chain_id,
            common_ref,
            timeout_gap,
            cycles_limit,
            cycles_price,
            interval,
            verifier_list,
            propose_ratio,
            prevote_ratio,
            precommit_ratio,
        })
    }
}
