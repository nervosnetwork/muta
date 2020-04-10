use std::mem;

use byteorder::{ByteOrder, LittleEndian};

use crate::fixed_codec::{FixedCodec, FixedCodecError};
use crate::types::{Bloom, Bytes, BytesMut, Hex};
use crate::ProtocolResult;

impl FixedCodec for bool {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode::<bool>(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode::<bool>(&bytes).map_err(FixedCodecError::Decoder)?)
    }
}

impl FixedCodec for u8 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode::<u8>(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode::<u8>(&bytes).map_err(FixedCodecError::Decoder)?)
    }
}

impl FixedCodec for u16 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode::<u16>(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode::<u16>(&bytes).map_err(FixedCodecError::Decoder)?)
    }
}

impl FixedCodec for u32 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode::<u32>(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode::<u32>(&bytes).map_err(FixedCodecError::Decoder)?)
    }
}

impl FixedCodec for u64 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode::<u64>(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode::<u64>(&bytes).map_err(FixedCodecError::Decoder)?)
    }
}

impl FixedCodec for u128 {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        let mut buf = [0u8; mem::size_of::<u128>()];
        LittleEndian::write_u128(&mut buf, *self);

        Ok(BytesMut::from(buf.as_ref()).freeze())
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(LittleEndian::read_u128(bytes.as_ref()))
    }
}

impl FixedCodec for String {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode::<String>(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode::<String>(&bytes).map_err(FixedCodecError::Decoder)?)
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

impl FixedCodec for Vec<u8> {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(self.clone()))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(bytes.to_vec())
    }
}

impl FixedCodec for Bloom {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        self.to_low_u64_le().encode_fixed()
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(Bloom::from_low_u64_le(u64::decode_fixed(bytes)?))
    }
}

impl FixedCodec for Hex {
    fn encode_fixed(&self) -> ProtocolResult<bytes::Bytes> {
        let bytes = self.as_string_trim0x().as_bytes().to_vec();
        Ok(bytes::Bytes::from(bytes))
    }

    fn decode_fixed(bytes: bytes::Bytes) -> ProtocolResult<Self> {
        let s = String::from_utf8(bytes.to_vec()).map_err(FixedCodecError::StringUTF8)?;
        Ok(Hex::from_string("0x".to_owned() + s.as_str())?)
    }
}
